use super::ENTRY_SIZE;
use core::mem::transmute;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct VolumeLabel {
    _entry_type: u8,
    pub(crate) character_count: u8,
    pub(crate) volume_label: [u8; 22], // TODO: might be better as [u16le; 11]
    _reserved: [u8; 8],
}

impl VolumeLabel {
    pub(crate) fn from_bytes(bytes: [u8; ENTRY_SIZE]) -> Self {
        // SAFETY: File is repr(C), and consists only of integers.
        unsafe { transmute(bytes) }
    }
}
