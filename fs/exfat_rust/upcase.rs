use crate::checksum::{calc_checksum_32, ChecksumType};
use crate::directory::{ExfatDirEntry, ExfatDirEntryReader};
use crate::external::BufferHead;
use crate::heap::cluster_to_sector;
use crate::superblock::{SbInfo, SbState, SuperBlockInfo};
use kernel::bindings::sector_t;
use kernel::prelude::*;
use kernel::{pr_err, pr_info, Error, Result};

//const NUM_UPCASE: usize = 2918;
const UTBL_COUNT: usize = 0x10000;

pub(crate) fn create_upcase_table(sbi: &mut SuperBlockInfo<'_>) -> Result {
    // TODO: scan the root directory set and read the allocation bitmap
    let sb_info = &mut sbi.info;
    let sb_state = sbi.state.as_mut().unwrap().get_mut();
    let root_dir = sb_info.boot_sector_info.root_dir;

    let upcase_entry = ExfatDirEntryReader::new(sb_info, sb_state, root_dir)?
        .find_map(|entry| match entry {
            Err(e) => Some(Err(e)),
            Ok(ExfatDirEntry::UpCaseTable(entry)) => Some(Ok(entry)),
            Ok(_) => None,
        })
        .transpose()?;

    match upcase_entry {
        Some(table) => {
            let sector = cluster_to_sector(sb_info, table.first_cluster.to_native());
            let num_sectors =
                ((table.data_length.to_native() - 1) >> sb_state.sb.s_blocksize_bits) + 1;

            match load_upcase_table(
                sb_info,
                sb_state,
                sector,
                num_sectors,
                table.table_checksum.to_native(),
            ) {
                e @ Err(Error::EIO) => return e,
                Err(err) => {
                    pr_info!("Failed to load upcase table, err: {:?}", err);
                    load_default_upcase_table(sbi)?;
                }
                Ok(()) => {}
            }
        }
        None => {
            load_default_upcase_table(sbi)?;
        }
    }

    Ok(())
}

fn load_default_upcase_table(_sbi: &mut SuperBlockInfo<'_>) -> Result {
    // TODO:
    todo!("Implement function");
}

#[allow(dead_code)] // TODO
fn load_upcase_table(
    sb_info: &mut SbInfo,
    sb_state: &mut SbState<'_>,
    mut sector: sector_t,
    mut num_sectors: u64,
    utbl_checksum: u32,
) -> Result {
    // TODO: we might want to rewrite this to use ClusterChain
    let sector_size = sb_state.sb.s_blocksize as usize;

    // unclear what type the UTF16 string should be
    // TODO: this might overflow the stack...
    let mut upcase_table = Box::try_new([0u16; UTBL_COUNT])?;

    let mut unicode_index = 0;
    let mut checksum = 0;
    let mut read_skip = false;
    num_sectors += sector;

    while sector < num_sectors {
        let bh = BufferHead::block_read(sb_state.sb, sector).ok_or_else(|| {
            pr_err!("Failed to read sector");
            Error::EIO
        })?;

        sector += 1;

        let b_data = &bh.bytes()[..sector_size];

        let mut last_index = b_data.len();
        for (i, entry) in b_data
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .enumerate()
        {
            // check if we've read the entire range of unicode values
            if unicode_index > 0xffff {
                last_index = i * 2;
                break;
            }

            if read_skip {
                // we're reading a compressed range of identity mapping
                // this entry tells us how many code points we should skip
                unicode_index += entry as usize;
                read_skip = false;
            } else if entry as usize == unicode_index {
                // this entry is an identity mapping, we can skip it
                unicode_index += 1;
            } else if entry == 0xffff {
                // the next entry will be a compressed range of identity mappings.
                read_skip = true;
            } else {
                // this entry is an actual upcase mapping, add it to the table
                upcase_table[unicode_index] = entry;
                unicode_index += 1;
            }
        }

        checksum = calc_checksum_32(&b_data[..last_index], checksum, ChecksumType::Default);
    }

    if unicode_index >= 0xffff && utbl_checksum == checksum as u32 {
        sb_info.upcase_table = Some(upcase_table);
        Ok(())
    } else {
        pr_err!("Failed to load upcase table");
        Err(Error::EINVAL)
    }
}
