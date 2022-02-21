#![allow(dead_code)] // TODO

mod allocation_bitmap;
mod file;
mod file_name;
mod stream_extension;
mod upcase;

use crate::fat::ClusterIndex;
use crate::heap::ClusterChain;
use allocation_bitmap::AllocationBitmap;
use core::iter::FusedIterator;
use file::File;
use file_name::FileName;
use kernel::bindings::super_block;
use kernel::Result;
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

/// All possible directory entries
pub(crate) enum DirEntry {
    // Critical primary
    AllocationBitmap(AllocationBitmap),
    UpCaseTable(UpCaseTable),
    VolumeLabel(ToDo),
    File(File),

    // Benign primary
    VolumeGuid(ToDo),
    TexFatPadding(ToDo),

    // Critical secondary
    StreamExtension(StreamExtension),
    FileName(FileName),

    // Benign secondary
    VendorExtension(ToDo),
    VendorAllocation(ToDo),
}

pub(crate) struct DirEntryReader<'a> {
    chain: ClusterChain<'a>,
    fused: bool,
}

impl<'a> DirEntryReader<'a> {
    pub(crate) fn new(sb: &'a super_block, index: ClusterIndex) -> Result<Self> {
        Ok(DirEntryReader {
            chain: ClusterChain::new(sb, index)?,
            fused: false,
        })
    }
}

impl FusedIterator for DirEntryReader<'_> {}
impl Iterator for DirEntryReader<'_> {
    type Item = Result<DirEntry>;

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
            ENTRY_TYPE_UPCASE => Some(Ok(DirEntry::UpCaseTable(UpCaseTable::from_bytes(buf)))),
            ENTRY_TYPE_BITMAP => Some(Ok(DirEntry::AllocationBitmap(
                AllocationBitmap::from_bytes(buf),
            ))),
            _ => self.next(), // TODO: remove this and implement remaining directory entries
        }
    }
}
