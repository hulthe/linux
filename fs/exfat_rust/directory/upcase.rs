use core::mem::{size_of, transmute};
use kernel::endian::{u32le, u64le};

/// An Upcase Table directory entry
#[repr(C)]
pub(crate) struct UpCaseTable {
    _entry_type: u8,
    _reserved1: [u8; 3],
    pub(crate) table_checksum: u32le,
    _reserved2: [u8; 12],
    pub(crate) first_cluster: u32le,
    pub(crate) data_length: u64le,
}

impl UpCaseTable {
    /// Convert to this type from the on-disk representation of a File
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        debug_assert_eq!(bytes.len(), size_of::<Self>());

        // SAFETY: File is repr(C) and has the same size as the byte array
        unsafe { transmute(bytes) }
    }
}
