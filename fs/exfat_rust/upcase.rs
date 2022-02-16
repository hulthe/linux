use crate::checksum::{calc_checksum_16, ChecksumType};
use crate::external::sb_bread;
use crate::get_exfat_sb_from_sb;
use crate::superblock::SuperBlockInfo;
use core::slice;
use kernel::bindings::{sector_t, super_block};
use kernel::prelude::*;
use kernel::{Error, Result};

const NUM_UPCASE: usize = 2918;
const UTBL_COUNT: usize = 0x10000;

pub(crate) fn load_upcase_table(
    sb: &mut super_block,
    mut sector: sector_t,
    mut num_sectors: u64,
    utbl_checksum: u32,
) -> Result {
    let sbi: &mut SuperBlockInfo = get_exfat_sb_from_sb!(sb);
    let sector_size = sb.s_blocksize as usize;

    // unclear what type the UTF16 string should be
    // TODO: this might overflow the stack...
    let mut upcase_table = Box::try_new([0u16; UTBL_COUNT])?;

    let mut unicode_index = 0;
    let mut checksum = 0;
    let mut skip = false;
    num_sectors += sector;

    while sector < num_sectors {
        let bh = sb_bread(sb, sector).ok_or_else(|| {
            // TODO: log err: failed to read sector
            Error::EIO
        })?;

        sector += 1;

        let b_data = bh.b_data as *const u8;
        let b_data = unsafe { slice::from_raw_parts(b_data, sector_size) };

        let mut last_index = 0;
        for (i, code_point) in b_data
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .enumerate()
        {
            last_index = i * 2;

            if unicode_index > 0xffff {
                break;
            }

            if skip {
                unicode_index += code_point as usize; // what???
                skip = false;
            } else if code_point as usize == unicode_index {
                unicode_index += 1;
            } else if code_point == 0xffff {
                skip = true;
            } else {
                upcase_table[unicode_index] = code_point;
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
