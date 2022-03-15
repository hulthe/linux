use crate::checksum::{calc_checksum_32, ChecksumType};
use crate::directory::{ExFatDirEntryKind, ExFatDirEntryReader};
use crate::external::BufferHead;
use crate::heap::cluster_to_sector;
use crate::superblock::{BootSectorInfo, SbState};
use kernel::bindings::sector_t;
use kernel::prelude::*;
use kernel::{pr_err, pr_info, Error, Result};

pub(crate) type UpcaseTable = Box<[u16]>;

//const NUM_UPCASE: usize = 2918;
const UTBL_COUNT: usize = 0x10000;

pub(crate) fn create_upcase_table(
    boot: &BootSectorInfo,
    sb_state: &mut SbState<'_>,
) -> Result<UpcaseTable> {
    let upcase_entry = ExFatDirEntryReader::new(boot, sb_state, boot.root_dir)?
        .find_map(|entry| match entry.map(|e| e.kind) {
            Err(e) => Some(Err(e)),
            Ok(ExFatDirEntryKind::UpCaseTable(entry)) => Some(Ok(entry)),
            Ok(_) => None,
        })
        .transpose()?;

    match upcase_entry {
        Some(table) => {
            let sector = cluster_to_sector(boot, table.first_cluster.to_native());
            let num_sectors =
                ((table.data_length.to_native() - 1) >> sb_state.sb.s_blocksize_bits) + 1;

            match load_upcase_table(
                sb_state,
                sector,
                num_sectors,
                table.table_checksum.to_native(),
            ) {
                e @ Err(Error::EIO) => return e,
                Err(err) => {
                    pr_info!("Failed to load upcase table, err: {:?}", err);
                    load_default_upcase_table()
                }
                Ok(upcase_table) => Ok(upcase_table),
            }
        }
        None => load_default_upcase_table(),
    }
}

fn load_default_upcase_table() -> Result<UpcaseTable> {
    // TODO:
    todo!("load default upcase table")
}

#[allow(dead_code)] // TODO
fn load_upcase_table(
    sb_state: &mut SbState<'_>,
    mut sector: sector_t,
    mut num_sectors: u64,
    utbl_checksum: u32,
) -> Result<UpcaseTable> {
    // TODO: we might want to rewrite this to use ClusterChain
    let sector_size = sb_state.sb.s_blocksize as usize;

    // unclear what type the UTF16 string should be
    // TODO: find a better way to initialzie upcase table
    //let mut upcase_table = Box::try_new([0u16; UTBL_COUNT])?; // might overflow stack
    let mut upcase_table = Vec::try_with_capacity(UTBL_COUNT)?;
    for _ in 0..UTBL_COUNT {
        upcase_table.try_push(0u16)?;
    }
    let mut upcase_table = upcase_table.try_into_boxed_slice()?;

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
        Ok(upcase_table)
    } else {
        pr_err!("Failed to load upcase table");
        Err(Error::EINVAL)
    }
}
