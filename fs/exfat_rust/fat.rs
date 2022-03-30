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

    /// The index which should be returned on the first call to next
    first: Option<ClusterIndex>,

    /// The index at which the next index is located
    read_next: Option<ClusterIndex>,

    block: Option<BufferHead>,
}

impl<'a> FatChainReader<'a> {
    pub(crate) fn new(boot: &'a BootSectorInfo, sb: &'a SuperBlock, index: ClusterIndex) -> Self {
        FatChainReader {
            boot,
            sb,
            first: Some(index),
            read_next: Some(index),
            block: None,
        }
    }
}

impl Iterator for FatChainReader<'_> {
    type Item = Result<ClusterIndex>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(first) = self.first.take() {
            return Some(Ok(first));
        }

        let index = self.read_next.take()?;

        let entry_size = size_of::<ClusterIndex>();
        let total_byte_offset = entry_size * index as usize;
        let sector_size = self.sb.s_blocksize as u64;
        let sector = self.boot.fat1_start_sector + total_byte_offset as u64 / sector_size;
        let byte_offset = total_byte_offset % sector_size as usize;

        let block = match &mut self.block {
            None => self.block.insert(BufferHead::block_read(self.sb, sector)?),
            Some(block) if block.sector() != sector => {
                self.block.insert(BufferHead::block_read(self.sb, sector)?)
            }
            Some(block) => block,
        };

        let bytes = &block.bytes()[byte_offset..][..entry_size];

        let next = ClusterIndex::from_le_bytes(bytes.try_into().unwrap());

        match next {
            FAT_ENTRY_FREE => {
                self.read_next = Some(index + 1);
                Some(Ok(index + 1))
            }
            FAT_ENTRY_EOF => None,
            FAT_ENTRY_BAD => Some(Err(Error::EIO)),

            _ if next > index => {
                self.read_next = Some(next);
                Some(Ok(next))
            }
            _ => {
                pr_err!("error: next FAT entry is smaller than current");
                Some(Err(Error::EIO))
            }
        }
    }
}
