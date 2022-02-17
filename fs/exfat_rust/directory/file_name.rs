use core::char::DecodeUtf16Error;
use core::mem::{size_of, transmute};

#[derive(Debug)]
pub(crate) struct FileName {
    _entry_type: u8,
    pub(crate) general_secondary_flags: u8,
    pub(crate) file_name: [u8; 30],
}

impl FileName {
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        debug_assert_eq!(bytes.len(), size_of::<Self>());

        // SAFETY: File is repr(C) and has the same size as the byte array
        unsafe { transmute(bytes) }
    }

    pub(crate) fn chars(&self) -> impl Iterator<Item = Result<char, DecodeUtf16Error>> + '_ {
        char::decode_utf16(
            self.file_name
                .chunks_exact(2)
                .map(|u16_b| u16::from_le_bytes([u16_b[0], u16_b[1]]))
                .take_while(|&b| b != 0),
        )
    }
}
