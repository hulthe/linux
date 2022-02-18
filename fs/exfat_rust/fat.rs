use crate::external::BufferHead;
use crate::superblock::SuperBlockInfo;
use core::mem::size_of;
use kernel::bindings::super_block;
use kernel::{Error, Result};

pub(crate) type ClusterIndex = u32;

const FAT_ENTRY_FREE: u32 = 0;
const FAT_ENTRY_BAD: u32 = 0xFFFFFFF7;
const FAT_ENTRY_EOF: u32 = 0xFFFFFFFF;

/// One entry in the FAT
#[derive(Debug)]
pub(crate) enum FatEntry {
    /// The corresponding cluster is bad
    Bad,

    /// The corresponding cluster the last of a cluster chain
    LastOfChain,

    /// This points to the *next* FatEntry in the given cluster chain.
    ///
    /// This must not point to a FatEntry that preceeds it
    NextFat(ClusterIndex),
}

pub(crate) struct FatChainReader<'a> {
    sb: &'a super_block,
    sbi: &'a SuperBlockInfo,
    next: Option<ClusterIndex>,
}

impl<'a> Iterator for FatChainReader<'a> {
    type Item = Result<ClusterIndex>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.next?;

        let entry_size = size_of::<ClusterIndex>();
        let total_byte_offset = entry_size * index as usize;
        let sector_size = self.sb.s_blocksize as u64;
        let sector = total_byte_offset as u64 / sector_size;
        let byte_offset = total_byte_offset % sector_size as usize;

        let block = BufferHead::block_read(self.sb, sector)?;
        let bytes = &block.bytes()[byte_offset..][..entry_size];

        let next = ClusterIndex::from_le_bytes(bytes.try_into().unwrap());
        match next {
            FAT_ENTRY_FREE | FAT_ENTRY_EOF => self.next = None,
            FAT_ENTRY_BAD => {
                self.next = None;
                return Some(Err(Error::EIO));
            }
            _ => self.next = Some(next),
        }

        Some(Ok(index))
    }
}
