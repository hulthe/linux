use crate::checksum::{calc_checksum_16, ChecksumType};
use crate::directory::{DirEntry, DirEntryReader};
use crate::external::BufferHead;
use crate::get_exfat_sb_from_sb;
use crate::heap::cluster_to_sector;
use crate::superblock::SuperBlockInfo;
use kernel::bindings::{sector_t, super_block};
use kernel::prelude::*;
use kernel::{Error, Result};

//const NUM_UPCASE: usize = 2918;
const UTBL_COUNT: usize = 0x10000;

pub(crate) fn create_upcase_table(sb: &mut super_block) -> Result {
    // TODO: scan the root directory set and read the allocation bitmap
    let sbi = get_exfat_sb_from_sb!(sb);
    let root_dir = sbi.boot_sector_info.root_dir;

    let upcase_entry = DirEntryReader::new(sb, root_dir)?
        .find_map(|entry| match entry {
            Err(e) => Some(Err(e)),
            Ok(DirEntry::UpCaseTable(entry)) => Some(Ok(entry)),
            Ok(_) => None,
        })
        .transpose()?;

    match upcase_entry {
        Some(table) => {
            let sector = cluster_to_sector(sbi, table.first_cluster.to_native());
            let num_sectors = ((table.data_length.to_native() - 1) >> sb.s_blocksize_bits) + 1;

            match load_upcase_table(sb, sector, num_sectors, table.table_checksum.to_native()) {
                e @ Err(Error::EIO) => return e,
                Err(_) => {
                    // TODO: load default table
                    todo!("load default table")
                }
                Ok(()) => Ok(()),
            }
        }
        None => {
            // TODO: load default table
            todo!("load default table")
        }
    }
}

#[allow(dead_code)] // TODO
fn load_upcase_table(
    sb: &mut super_block,
    mut sector: sector_t,
    mut num_sectors: u64,
    utbl_checksum: u32,
) -> Result {
    // TODO: we might want to rewrite this to use ClusterChain

    let sbi: &mut SuperBlockInfo = get_exfat_sb_from_sb!(sb);
    let sector_size = sb.s_blocksize as usize;

    // unclear what type the UTF16 string should be
    // TODO: this might overflow the stack...
    let mut upcase_table = Box::try_new([0u16; UTBL_COUNT])?;

    let mut unicode_index = 0;
    let mut checksum = 0;
    let mut read_skip = false;
    num_sectors += sector;

    while sector < num_sectors {
        let bh = BufferHead::block_read(sb, sector).ok_or_else(|| {
            // TODO: log err: failed to read sector
            Error::EIO
        })?;

        sector += 1;

        let b_data = &bh.bytes()[..sector_size];

        let mut last_index = 0;
        for (i, entry) in b_data
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .enumerate()
        {
            last_index = i * 2;

            // check if we've read the entire range of unicode values
            if unicode_index > 0xffff {
                break;
            }

            if read_skip {
                // we're reading a compressed range of identity mapping
                // this entry tells us how many code points we should skip
                unicode_index += entry as usize;
                read_skip = false;
            } else if entry == 0xffff {
                // the next entry will be a compressed range of identity mappings.
                read_skip = true;
            } else if entry as usize == unicode_index {
                // this entry is an identity mapping, we can skip it
                unicode_index += 1;
            } else {
                // this entry is an actual upcase mapping, add it to the table
                upcase_table[unicode_index] = entry;
                unicode_index += 1;
            }
        }

        checksum = calc_checksum_16(&b_data[..last_index], checksum, ChecksumType::Default);
    }

    if unicode_index >= 0xffff && utbl_checksum == checksum as u32 {
        sbi.vol_utbl = Some(upcase_table);
        Ok(())
    } else {
        // TODO: log error: failed to load upcase table ...
        Err(Error::EINVAL)
    }
}
