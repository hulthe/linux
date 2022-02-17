use core::mem::{size_of, transmute};
use kernel::endian::{u32le, u64le};

#[repr(C)]
pub(crate) struct AllocationBitmap {
    _entry_type: u8,
    pub(crate) bitmap_flags: u8,
    _reserved: [u8; 18],
    pub(crate) first_cluster: u32le,
    pub(crate) data_length: u64le,
}

impl AllocationBitmap {
    /// Convert to this type from the on-disk representation of an AllocationBitmap
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        debug_assert_eq!(bytes.len(), size_of::<Self>());

        // SAFETY: File is repr(C) and has the same size as the byte array
        unsafe { transmute(bytes) }
    }
}
