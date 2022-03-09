#![allow(dead_code)] // TODO

pub(crate) mod allocation_bitmap;
pub(crate) mod file;
pub(crate) mod file_name;
pub(crate) mod stream_extension;
pub(crate) mod upcase;

use crate::fat::ClusterIndex;
use crate::heap::ClusterChain;
use crate::superblock::{SbInfo, SbState};
use alloc::string::String;
use allocation_bitmap::AllocationBitmap;
use core::iter::FusedIterator;
use core::ops::Range;
use file::{File, FileAttributes};
use file_name::FileName;
use kernel::bindings::timespec64;
use kernel::prelude::*;
use kernel::{pr_err, Error, Result};
use stream_extension::StreamExtension;
use upcase::UpCaseTable;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ToDo;

/// The size of a directory  in bytes
const ENTRY_SIZE: usize = 32;

// TODO: copied constants from C, pls rename at your earliest convenience. thank
const ENTRY_TYPE_END_OF_DIRECTORY: u8 = 0x00;
const ENTRY_TYPE_DELETED: Range<u8> = 0x01..0x80;
const ENTRY_TYPE_INVAL: u8 = 0x80;
const ENTRY_TYPE_BITMAP: u8 = 0x81;
const ENTRY_TYPE_UPCASE: u8 = 0x82;
const ENTRY_TYPE_VOLUME: u8 = 0x83;
const ENTRY_TYPE_FILE: u8 = 0x85;
const ENTRY_TYPE_GUID: u8 = 0xA0;
const ENTRY_TYPE_PADDING: u8 = 0xA1;
const ENTRY_TYPE_ACLTAB: u8 = 0xA2;
const ENTRY_TYPE_STREAM: u8 = 0xC0;
const ENTRY_TYPE_NAME: u8 = 0xC1;
const ENTRY_TYPE_ACL: u8 = 0xC2;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ExFatDirEntry {
    /// The start of the cluster chain with the directory set which contains this entry
    pub(crate) cluster: u32,

    /// The index of this entry within the directory set
    pub(crate) index: u32,

    pub(crate) kind: ExFatDirEntryKind,
}

/// All possible raw exfat directory entries
#[derive(Clone, Copy, Debug)]
pub(crate) enum ExFatDirEntryKind {
    Deleted,

    // Critical primary
    AllocationBitmap(AllocationBitmap),
    UpCaseTable(UpCaseTable),
    VolumeLabel(ToDo), // TODO
    File(File),

    // Benign primary
    VolumeGuid(ToDo),    // TODO
    TexFatPadding(ToDo), // TODO

    // Critical secondary
    StreamExtension(StreamExtension),
    FileName(FileName),

    // Benign secondary
    VendorExtension(ToDo),  // TODO
    VendorAllocation(ToDo), // TODO
}

pub(crate) struct ExFatDirEntryReader<'a> {
    chain: ClusterChain<'a>,
    fused: bool,

    /// Tracks indices of the ExFatDirEntry:s we're reading
    index: u32,
}

impl<'a> ExFatDirEntryReader<'a> {
    pub(crate) fn new(
        sb_info: &'a SbInfo,
        sb_state: &'a SbState<'a>,
        index: ClusterIndex,
    ) -> Result<Self> {
        Ok(Self {
            chain: ClusterChain::new(sb_info, sb_state, index)?,
            fused: false,
            index: 0,
        })
    }
}

impl FusedIterator for ExFatDirEntryReader<'_> {}
impl Iterator for ExFatDirEntryReader<'_> {
    type Item = Result<ExFatDirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.fused {
            return None;
        }

        let index = self.index;
        self.index += 1;

        let mut buf = [0u8; 32];

        if let Err(e) = self.chain.read_exact(&mut buf) {
            self.fused = true;
            return Some(Err(e));
        }

        let entry_type = buf[0];

        use ExFatDirEntryKind as Entry;
        let kind = match entry_type {
            ENTRY_TYPE_END_OF_DIRECTORY => {
                // we have reached the end of the directory set
                self.fused = true;
                return None;
            }
            t if ENTRY_TYPE_DELETED.contains(&t) => Entry::Deleted,
            ENTRY_TYPE_UPCASE => Entry::UpCaseTable(UpCaseTable::from_bytes(buf)),
            ENTRY_TYPE_BITMAP => Entry::AllocationBitmap(AllocationBitmap::from_bytes(buf)),
            ENTRY_TYPE_FILE => Entry::File(File::from_bytes(buf)),
            ENTRY_TYPE_STREAM => Entry::StreamExtension(StreamExtension::from_bytes(buf)),
            ENTRY_TYPE_NAME => Entry::FileName(FileName::from_bytes(buf)),
            _ => {
                pr_info!("ExFatDirEntryReader: skipping unknown directory entry: {entry_type:x}");
                return self.next(); // TODO: remove this and implement remaining directory entries
            }
        };

        Some(Ok(ExFatDirEntry {
            cluster: self.chain.start_cluster(),
            index,
            kind,
        }))
    }
}

/// High-level directory entry
pub(crate) struct DirEntry {
    /// The start of the cluster chain which contains the data for this DirEntry
    pub(crate) data_cluster: ClusterIndex,

    /// The length of the data in the cluster chain
    pub(crate) data_length: u64,

    /// The start of the cluster chain which has the directory set that contains this entry.
    pub(crate) cluster: u32,

    /// The index of this entry in the directory set
    ///
    /// Specifically, the index of the ExFatDirEntry File that marks the start of this DirEntry
    pub(crate) index: u32,

    pub(crate) name: String,

    pub(crate) attrs: FileAttributes,

    pub(crate) create_time: timespec64,
    pub(crate) access_time: timespec64,
    pub(crate) modified_time: timespec64,
}

pub(crate) struct DirEntryReader<'a> {
    sb_info: &'a SbInfo,
    entries: ExFatDirEntryReader<'a>,
}

impl<'a> DirEntryReader<'a> {
    pub(crate) fn new(
        sb_info: &'a SbInfo,
        sb_state: &'a SbState<'a>,
        index: ClusterIndex,
    ) -> Result<Self> {
        Ok(Self {
            sb_info,
            entries: ExFatDirEntryReader::new(sb_info, sb_state, index)?,
        })
    }
}

impl FusedIterator for DirEntryReader<'_> {}
impl Iterator for DirEntryReader<'_> {
    type Item = Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        let file = self
            .entries
            .find_map(|entry| match entry.map(|e| (e, e.kind)) {
                Err(e) => Some(Err(e)),
                Ok((entry, ExFatDirEntryKind::File(file))) => Some(Ok((entry, file))),
                Ok(_) => None,
            })?;

        let (file_entry, file) = match file {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };

        let stream_ext = match self.entries.next() {
            Some(Err(e)) => {
                pr_err!("Failed to retrieve next DirEntry, err {:?}", e);
                return Some(Err(e));
            }
            Some(Ok(ExFatDirEntry {
                kind: ExFatDirEntryKind::StreamExtension(entry),
                ..
            })) => entry,
            None => {
                pr_err!("ExFatDirEntryReader: expected StreamExtension, found nothing");
                return Some(Err(Error::EIO)); // TODO: not sure which error is appropriate here
            }
            Some(Ok(v)) => {
                pr_err!("Unknown entry: {:?}", v);
                return Some(Err(Error::EIO)); // TODO: not sure which error is appropriate here
            }
        };

        let name_length = stream_ext.name_length as usize;

        // one FileName contains up to 15 UTF-16 code points
        let number_of_file_name_entries = (name_length - 1) / 15 + 1;

        let mut name_buffer: Vec<u8> = Vec::new();
        if let Err(e) = name_buffer.try_reserve(name_length) {
            pr_err!("Failed to allocate namebuffer");
            return Some(Err(e.into()));
        }

        for _ in 0..number_of_file_name_entries {
            let file_name_entry = match self.entries.next() {
                Some(Err(e)) => {
                    pr_err!("Failed to retrieve next DirEntry, err {:?}", e);
                    return Some(Err(e));
                }
                Some(Ok(ExFatDirEntry {
                    kind: ExFatDirEntryKind::FileName(entry),
                    ..
                })) => entry,
                None => {
                    pr_err!("ExFatDirEntryReader: expected StreamExtension, found nothing");
                    return Some(Err(Error::EIO)); // TODO: not sure which error is appropriate here
                }
                Some(Ok(v)) => {
                    pr_err!("Unknown entry: {:?}", v);
                    return Some(Err(Error::EIO)); // TODO: not sure which error is appropriate here
                }
            };

            for c in file_name_entry.chars() {
                let c = match c {
                    Err(_e) => return Some(Err(Error::EIO)), // TODO: not sure which error
                    Ok(c) => c,
                };

                let mut utf8_buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut utf8_buf);
                if let Err(e) = name_buffer.try_extend_from_slice(encoded.as_bytes()) {
                    pr_err!("Failed to append to namebuffer");
                    return Some(Err(e.into()));
                }
            }
        }

        let name = match String::from_utf8(name_buffer) {
            Ok(v) => v,
            Err(err) => {
                pr_err!("Failed to convert namebuffer to utf8, err {}", err);
                return Some(Err(Error::EINVAL)); // TODO: Not sure about error...
            }
        };

        let dir_entry = DirEntry {
            data_cluster: stream_ext.first_cluster.to_native(),
            data_length: stream_ext.data_length.to_native(),

            cluster: file_entry.cluster,
            index: file_entry.index,

            attrs: file.file_attributes,
            name,

            create_time: file.create_time(self.sb_info),
            access_time: file.access_time(self.sb_info),
            modified_time: file.modified_time(self.sb_info),
        };
        Some(Ok(dir_entry))
    }
}
