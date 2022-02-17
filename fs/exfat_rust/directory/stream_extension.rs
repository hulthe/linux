use core::mem::{size_of, transmute};
use kernel::endian::{u16le, u32le, u64le};

#[repr(C)]
pub(crate) struct StreamExtension {
    _entry_type: u8,

    pub(crate) general_secondary_flags: u8,

    _reserved1: u8,

    /// The length of the UTF-16 string in the subsequent FileName directory set entries
    pub(crate) name_length: u8,

    pub(crate) name_hash: u16le,

    _reserved2: [u8; 2],

    pub(crate) valid_data_length: u64le,

    _reserved3: [u8; 4],

    pub(crate) first_cluster: u32le,

    pub(crate) data_length: u64le,
}

impl StreamExtension {
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        debug_assert_eq!(bytes.len(), size_of::<Self>());

        // SAFETY: File is repr(C) and has the same size as the byte array
        unsafe { transmute(bytes) }
    }
}
