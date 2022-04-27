use super::EXFAT_DIR_ENTRY_SIZE;
use core::char::DecodeUtf16Error;
use core::mem::transmute;
use kernel::endian::u16le;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct FileName {
    _entry_type: u8,
    pub(crate) general_secondary_flags: u8,
    pub(crate) file_name: [u16le; 15],
}

impl FileName {
    pub(crate) fn from_bytes(bytes: [u8; EXFAT_DIR_ENTRY_SIZE]) -> Self {
        // SAFETY: File is repr(C), and consists only of integers.
        unsafe { transmute(bytes) }
    }

    /// The number of UTF-16 code points that this can contain
    pub(crate) const fn capacity(&self) -> usize {
        self.file_name.len()
    }

    pub(crate) fn chars(&self) -> impl Iterator<Item = Result<char, DecodeUtf16Error>> + '_ {
        char::decode_utf16(
            self.file_name
                .iter()
                .map(|le| le.to_native())
                .take_while(|&b| b != 0),
        )
    }
}
