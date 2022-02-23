use crate::directory::{DirEntry, DirEntryReader};
use crate::get_exfat_sb_from_sb;
use crate::heap::ClusterChain;
use crate::superblock::{SuperBlock, SuperBlockInfo};
use crate::BITS_PER_BYTE;
use alloc::boxed::Box;
use alloc::vec::Vec;
use kernel::{pr_err, Error, Result};

pub(crate) struct AllocationBitmap {
    // INVARIANT: the trailing bits in the last byte must be 0
    bitmap: Box<[u8]>,

    allocation_count: u64,

    #[allow(dead_code)] // TODO
    cluster_count: u32,
}

impl AllocationBitmap {
    //pub(crate) fn free_cluster_count(&self) -> u64 {
    //    self.cluster_count as u64 - self.allocated_cluster_count()
    //}

    #[allow(dead_code)] // TODO
    pub(crate) fn allocated_cluster_count(&self) -> u64 {
        self.allocation_count
    }

    fn count_allocations(&mut self) {
        self.allocation_count = self.bitmap.iter().map(|&b| b.count_ones() as u64).sum();
    }
}

pub(crate) fn load_allocation_bitmap(sb: &mut SuperBlock) -> Result {
    let sbi = get_exfat_sb_from_sb!(sb);
    let root_dir = sbi.boot_sector_info.root_dir;

    let bitmap_entry = DirEntryReader::new(sb, root_dir)?
        .find_map(|entry| match entry {
            Err(e) => Some(Err(e)),
            Ok(DirEntry::AllocationBitmap(entry)) => Some(Ok(entry)),
            Ok(_) => None,
        })
        .ok_or(Error::EINVAL)??;

    // flags specify which fat this allocation bitmap refers to
    if bitmap_entry.bitmap_flags != 0x0 {
        // return if this allocation bitmap refers to the second FAT
        return Err(Error::EINVAL);
    }

    sbi.map_clu = bitmap_entry.first_cluster.to_native();
    let cluster_count = sbi.boot_sector_info.cluster_count();
    let size = bitmap_entry.data_length.to_native();
    let required_size = ((cluster_count - 1) as u64 / BITS_PER_BYTE as u64) + 1;

    if size != required_size {
        // TODO: logging
        pr_err!("bogus allocation bitmap size (need {required_size}, got {size})");

        // Only allowed when bogus allocation bitmap size is large
        if size < required_size {
            return Err(Error::EIO);
        }
    }

    // TODO: the C version just keeps a bunch of BufferHead:s in a map
    // here, we're copying all the bytes into a new array instead. might hamper performance.

    let mut bitmap = Vec::new();

    // normally this would just be vec![0; required_size], but we have to account for fallible
    // allocations. TODO: figure out how to avoid this awful loop
    for _ in 0..required_size {
        bitmap.try_push(0)?;
    }

    ClusterChain::new(sb, sbi.map_clu)?.read_exact(&mut bitmap)?;

    // make sure the trailing bits are 0
    let trailing_bits = required_size * BITS_PER_BYTE as u64 - cluster_count as u64;
    let trailing_mask = u8::MAX << trailing_bits;
    bitmap[required_size as usize - 1] &= trailing_mask;

    let bitmap = bitmap.try_into_boxed_slice()?;

    let mut bitmap = AllocationBitmap {
        bitmap,
        cluster_count,
        allocation_count: 0,
    };
    bitmap.count_allocations();

    sbi.allocation_bitmap = Some(bitmap);

    Ok(())
}
