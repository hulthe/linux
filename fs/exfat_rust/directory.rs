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
use file::{File, FileAttributes};
use file_name::FileName;
use kernel::{Error, Result};
use stream_extension::StreamExtension;
use upcase::UpCaseTable;

pub(crate) struct ToDo;

/// The size of a directory  in bytes
const ENTRY_SIZE: usize = 32;

// copied constants from C, pls rename at your earliest convenience. thank
const ENTRY_TYPE_UNUSED: u8 = 0x00;
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

/// All possible raw exfat directory entries
pub(crate) enum ExfatDirEntry {
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

pub(crate) struct ExfatDirEntryReader<'a> {
    chain: ClusterChain<'a>,
    fused: bool,
}

impl<'a> ExfatDirEntryReader<'a> {
    pub(crate) fn new(
        sb_info: &'a SbInfo,
        sb_state: &'a SbState<'a>,
        index: ClusterIndex,
    ) -> Result<Self> {
        Ok(Self {
            chain: ClusterChain::new(sb_info, sb_state, index)?,
            fused: false,
        })
    }
}

impl FusedIterator for ExfatDirEntryReader<'_> {}
impl Iterator for ExfatDirEntryReader<'_> {
    type Item = Result<ExfatDirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.fused {
            return None;
        }

        let mut buf = [0u8; 32];

        if let Err(e) = self.chain.read_exact(&mut buf) {
            self.fused = true;
            return Some(Err(e));
        }

        let entry_type = buf[0];

        match entry_type {
            ENTRY_TYPE_UNUSED => {
                self.fused = true;
                None
            }
            ENTRY_TYPE_UPCASE => Some(Ok(ExfatDirEntry::UpCaseTable(UpCaseTable::from_bytes(buf)))),
            ENTRY_TYPE_BITMAP => Some(Ok(ExfatDirEntry::AllocationBitmap(
                AllocationBitmap::from_bytes(buf),
            ))),
            ENTRY_TYPE_FILE => Some(Ok(ExfatDirEntry::File(File::from_bytes(buf)))),
            _ => self.next(), // TODO: remove this and implement remaining directory entries
        }
    }
}

/// High-level directory entry
#[derive(Debug)]
pub(crate) struct DirEntry {
    cluster: ClusterIndex,
    data_length: u64,

    attrs: FileAttributes,
}

pub(crate) struct DirEntryReader<'a> {
    entries: ExfatDirEntryReader<'a>,
}

impl<'a> DirEntryReader<'a> {
    pub(crate) fn new(
        sb_info: &'a SbInfo,
        sb_state: &'a SbState<'a>,
        index: ClusterIndex,
    ) -> Result<Self> {
        Ok(Self {
            entries: ExfatDirEntryReader::new(sb_info, sb_state, index)?,
        })
    }
}

impl FusedIterator for DirEntryReader<'_> {}
impl Iterator for DirEntryReader<'_> {
    type Item = Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        let file = self.entries.find_map(|entry| match entry {
            Err(e) => Some(Err(e)),
            Ok(ExfatDirEntry::File(entry)) => Some(Ok(entry)),
            Ok(_) => None,
        })?;

        let file = match file {
            Ok(f) => f,
            Err(e) => return Some(Err(e)),
        };

        let stream_ext = match self.entries.next() {
            Some(Err(e)) => return Some(Err(e)),
            Some(Ok(ExfatDirEntry::StreamExtension(entry))) => entry,
            _ => return Some(Err(Error::EIO)), // TODO: not sure which error is appropriate here
        };

        let name_length = stream_ext.name_length as usize;

        let mut name = String::new();
        if let Err(e) = name.try_reserve(name_length * 2 /* File name is UTF16 */) {
            return Some(Err(e.into()));
        }

        // one FileName contains up to 15 UTF-16 code points
        let number_of_file_name_entries = (name_length - 1) / 15 + 1;

        for _ in 1..=number_of_file_name_entries {
            let file_name_entry = match self.entries.next() {
                Some(Err(e)) => return Some(Err(e)),
                Some(Ok(ExfatDirEntry::FileName(entry))) => entry,
                _ => return Some(Err(Error::EIO)), // TODO: not sure which error is appropriate here
            };

            // TODO: save file name in a buffer somewhere idk
            let _ = file_name_entry;
        }

        Some(Ok(DirEntry {
            cluster: stream_ext.first_cluster.to_native(),
            data_length: stream_ext.data_length.to_native(),
            attrs: file.file_attributes,
        }))
    }
}
