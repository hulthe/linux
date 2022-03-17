use crate::external::BufferHead;
use crate::superblock::{BootSectorInfo, SuperBlock};
use core::mem::size_of;
use kernel::{pr_err, Error, Result};

pub(crate) type ClusterIndex = u32;

const FAT_ENTRY_FREE: u32 = 0;
const FAT_ENTRY_BAD: u32 = 0xFFFFFFF7;
const FAT_ENTRY_EOF: u32 = 0xFFFFFFFF;

pub(crate) struct FatChainReader<'a> {
    boot: &'a BootSectorInfo,
    sb: &'a SuperBlock,
    next: Option<ClusterIndex>,
}

impl<'a> FatChainReader<'a> {
    pub(crate) fn new(boot: &'a BootSectorInfo, sb: &'a SuperBlock, index: ClusterIndex) -> Self {
        FatChainReader {
            boot,
            sb,
            next: Some(index),
        }
    }
}

impl Iterator for FatChainReader<'_> {
    type Item = Result<ClusterIndex>;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.next?;

        let entry_size = size_of::<ClusterIndex>();
        let total_byte_offset = entry_size * index as usize;
        let sector_size = self.sb.s_blocksize as u64;
        let sector = self.boot.fat1_start_sector + total_byte_offset as u64 / sector_size;
        let byte_offset = total_byte_offset % sector_size as usize;

        let block = BufferHead::block_read(self.sb, sector)?;
        let bytes = &block.bytes()[byte_offset..][..entry_size];

        let next = ClusterIndex::from_le_bytes(bytes.try_into().unwrap());

        match next {
            FAT_ENTRY_FREE => self.next = Some(index + 1),
            FAT_ENTRY_EOF => self.next = None,
            FAT_ENTRY_BAD => {
                self.next = None;
                return Some(Err(Error::EIO));
            }
            _ if next > index => self.next = Some(next),
            _ => {
                pr_err!("error: next FAT entry is smaller than current");
                self.next = None;
                return Some(Err(Error::EIO));
            }
        }

        Some(Ok(index))
    }
}
