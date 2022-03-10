use super::ENTRY_SIZE;
use crate::superblock::SbInfo;
use core::mem::transmute;
use kernel::bindings::{mktime64, timespec64, NSEC_PER_MSEC, S_IFDIR, S_IFREG};
use kernel::endian::u16le;

// TODO: expand on this type
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct TimeStamp {
    time: u16le,
    date: u16le,
}

/// A File directory entry
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct File {
    _entry_type: u8,
    pub(crate) secondary_count: u8,
    pub(crate) set_checksum: u16le,
    pub(crate) file_attributes: FileAttributes,
    _reserved1: [u8; 2],
    pub(crate) create_timestamp: TimeStamp,
    pub(crate) last_modified_timestamp: TimeStamp,
    pub(crate) last_accessed_timestamp: TimeStamp,
    pub(crate) create_10ms_increments: u8,
    pub(crate) last_modified_10ms_increments: u8,
    pub(crate) create_utc_offset: u8,
    pub(crate) last_modified_utc_offset: u8,
    pub(crate) last_accessed_utc_offset: u8,
    _reserved2: [u8; 7],
}

/// The attribute bits of a [File]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct FileAttributes {
    bits: u16le,
}

impl File {
    /// Convert to this type from the on-disk representation of a File
    pub(crate) fn from_bytes(bytes: [u8; ENTRY_SIZE]) -> Self {
        // SAFETY: File is repr(C), and consists only of integers.
        unsafe { transmute(bytes) }
    }

    pub(crate) fn create_time(&self, sb_info: &SbInfo) -> timespec64 {
        self.create_timestamp.to_unix_time(
            sb_info,
            self.create_10ms_increments,
            self.create_utc_offset,
        )
    }

    pub(crate) fn modified_time(&self, sb_info: &SbInfo) -> timespec64 {
        self.last_modified_timestamp.to_unix_time(
            sb_info,
            self.last_modified_10ms_increments,
            self.last_modified_utc_offset,
        )
    }

    pub(crate) fn access_time(&self, sb_info: &SbInfo) -> timespec64 {
        self.last_accessed_timestamp
            .to_unix_time(sb_info, 0, self.last_accessed_utc_offset)
    }
}

impl FileAttributes {
    pub(crate) fn from_u16(num: u16) -> Self {
        FileAttributes { bits: num.into() }
    }

    pub(crate) fn read_only(&self) -> bool {
        bit::<0>(self.bits.to_native())
    }

    pub(crate) fn hidden(&self) -> bool {
        bit::<1>(self.bits.to_native())
    }

    pub(crate) fn system(&self) -> bool {
        bit::<2>(self.bits.to_native())
    }

    pub(crate) fn directory(&self) -> bool {
        bit::<4>(self.bits.to_native())
    }

    pub(crate) fn archive(&self) -> bool {
        bit::<5>(self.bits.to_native())
    }

    // Convert exFAT file attributes to the UNIX mode
    pub(crate) fn to_unix(&self, mut mode: u16, sb_info: &SbInfo) -> u16 {
        if self.directory() {
            return (mode & !sb_info.options.fs_dmask) | (S_IFDIR as u16);
        }

        if self.read_only() {
            mode &= !0o222;
        }

        (mode & !sb_info.options.fs_fmask) | (S_IFREG as u16)
    }
}

struct BitPos {
    offset: u32,
    size: u32,
}

const SECS_PER_MIN: i64 = 60;
const NSEC_PER_CSEC: i64 = 10 * NSEC_PER_MSEC as i64;
const TZ_VALID_BIT: u8 = 0b10000000;

impl TimeStamp {
    fn to_unix_time(&self, sbi: &SbInfo, time_cs: u8, tz: u8) -> timespec64 {
        const fn extract_bits(pos: BitPos, bits: u16) -> u32 {
            let mask = u32::MAX >> (u32::BITS - pos.size);
            (bits as u32 >> pos.offset) & mask
        }

        const SECONDS_POS: BitPos = BitPos { offset: 0, size: 5 };
        const MINUTE_POS: BitPos = BitPos { offset: 5, size: 6 };
        const HOUR_POS: BitPos = BitPos {
            offset: 11,
            size: 5,
        };

        const DAY_POS: BitPos = BitPos { offset: 0, size: 5 };
        const MONTH_POS: BitPos = BitPos { offset: 5, size: 4 };
        const YEAR_POS: BitPos = BitPos { offset: 9, size: 7 };

        let time = self.time.to_native();
        let double_seconds = extract_bits(SECONDS_POS, time);
        let seconds = double_seconds * 2;
        let minute = extract_bits(MINUTE_POS, time);
        let hour = extract_bits(HOUR_POS, time);

        let date = self.date.to_native();
        let day = extract_bits(DAY_POS, date);
        let month = extract_bits(MONTH_POS, date);
        let year = 1980 + extract_bits(YEAR_POS, date);

        let mut ts = timespec64 {
            tv_sec: unsafe { mktime64(year, month, day, hour, minute, seconds) },
            tv_nsec: 0,
        };

        // time_cs field represents 0 - 199cs (1990ms)
        if time_cs != 0 {
            ts.tv_sec += (time_cs / 100) as i64;
            ts.tv_nsec = (time_cs % 100) as i64 * NSEC_PER_CSEC;
        }

        if (tz & TZ_VALID_BIT) != 0 {
            // Adjust timezone to UTC0
            adjust_tz(&mut ts, tz & !TZ_VALID_BIT);
        } else {
            // Convert from local time to UTC using time_offset
            ts.tv_sec -= sbi.options.time_offset as i64 * SECS_PER_MIN;
        }

        ts
    }
}

/// I have no idea (TODO)
fn adjust_tz(ts: &mut timespec64, offset: u8) {
    let timezone_sec = |offset| offset as i64 * 15 * SECS_PER_MIN;

    if offset <= 0x3F {
        ts.tv_sec -= timezone_sec(offset);
    } else {
        ts.tv_sec += timezone_sec(0x80 - offset);
    }
}

// Directory
pub(crate) const ROOT_FILE_ATTRIBUTE: u16 = 0x0010;

/// Read a single bit of an integer
#[inline]
const fn bit<const N: usize>(byte: u16) -> bool {
    (byte >> N) & 1 == 1
}
