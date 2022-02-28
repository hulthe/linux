use crate::superblock::SuperBlockInfo;
use core::mem::{size_of, transmute};
use kernel::bindings::{S_IFDIR, S_IFREG};
use kernel::endian::u16le;

// TODO: expand on this type
#[repr(C)]
pub(crate) struct TimeStamp([u8; 4]);

/// A File directory entry
#[repr(C)]
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
pub(crate) struct FileAttributes {
    bits: u16le,
}

impl File {
    /// Convert to this type from the on-disk representation of a File
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        debug_assert_eq!(bytes.len(), size_of::<Self>());

        // SAFETY: File is repr(C) and has the same size as the byte array
        unsafe { transmute(bytes) }
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
    pub(crate) fn to_unix(&self, mut mode: u16, sb_info: &SuperBlockInfo) -> u16 {
        if self.directory() {
            return (mode & !sb_info.options.fs_dmask) | (S_IFDIR as u16);
        }

        if self.read_only() {
            mode &= !0o222;
        }

        (mode & !sb_info.options.fs_fmask) | (S_IFREG as u16)
    }
}

// Directory
pub(crate) const ROOT_FILE_ATTRIBUTE: u16 = 0x0010;

/// Read a single bit of an integer
#[inline]
const fn bit<const N: usize>(byte: u16) -> bool {
    (byte >> N) & 1 == 1
}
