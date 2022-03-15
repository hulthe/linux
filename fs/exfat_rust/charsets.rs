#![allow(dead_code)]

use crate::superblock::SbInfo;
use kernel::bindings::{utf16_endian_UTF16_HOST_ENDIAN, utf8s_to_utf16s};
use kernel::endian::u16le;
use kernel::prelude::*;
use kernel::{pr_err, Error, Result};

// TODO: These constants should probably be moved somewhere else.
// Max length of a filename excluding NULL
pub(crate) const MAX_NAME_LEN: i32 = 255;
// Max size of multi-byte character
pub(crate) const MAX_CHARSET_SIZE: u32 = 6;

pub(crate) struct UTF16String(pub(crate) [u16; MAX_NAME_LEN as usize + 3]);

/*
 * Allow full-width illegal characters :
 * "MS windows 7" supports full-width-invalid-name-characters.
 * So we should check half-width-invalid-name-characters(ASCII) only
 * for compatibility.
 *
 * " * / : < > ? \ |
 */
const BAD_UNICODE_CHARACTERS: &[u16] = &[
    0x0022, 0x002A, 0x002F, 0x003A, 0x003C, 0x003E, 0x003F, 0x005C, 0x007C,
];

enum NlsNameMode {
    NoLossy = 0,
    Lossy = 1,
    Overlen = 2,
}

impl UTF16String {
    pub(crate) fn from_nls(sb_info: &SbInfo, nls_string: String, p_lossy: bool) -> Result<String> {
        if sb_info.options.utf8 {
            todo!("Non UTF8 modes are not supported");
        }

        // FIXME: Figure out why + 2.
        let u16_length = MAX_NAME_LEN + 2;

        // + 3 for NULL and converting(?)
        let mut utf16_string = [0u16; MAX_NAME_LEN as usize + 3];

        let nls_string_bytes = nls_string.as_bytes();

        // SAFETY: The function returns a negative number if an error occured
        // otherwise, it should write the utf16 variant of the string to utf16_string.
        // nls_string_bytes: the function should not store a copy of the reference and thus it does not matter if it is dropped after this point.
        // utf16_string: the contract with the function is that if no error is returned, utf16_string SHOULD contain the converted value.
        let length_or_err = unsafe {
            utf8s_to_utf16s(
                nls_string_bytes.as_ptr(),
                nls_string.len() as i32,
                utf16_endian_UTF16_HOST_ENDIAN,
                utf16_string.as_mut_ptr(),
                u16_length as i32,
            )
        };

        if length_or_err < 0 {
            return Err(Error::EINVAL);
        }

        let length = length_or_err;

        if length > MAX_NAME_LEN {
            pr_err!(
                "failed to convert utf8 to utf16, length : {} > {}",
                length_or_err,
                MAX_NAME_LEN
            );
            return Err(Error::ENAMETOOLONG);
        }

        // TODO: Implement
        let mut _lossy = NlsNameMode::NoLossy as u32;

        const UPPERCASE_NAME_LEN: usize = MAX_NAME_LEN as usize + 1;
        let mut uppercase_name = [u16le::from(0); UPPERCASE_NAME_LEN];

        let length = length as usize;

        for i in 0..length {
            let char16: u16 = utf16_string[i];
            if char16 < 0x0020 || BAD_UNICODE_CHARACTERS.contains(&char16) {
                _lossy |= NlsNameMode::Lossy as u32;
            }

            uppercase_name[i] = to_upper(sb_info, char16).into();
        }

        // Append NULL terminator
        utf16_string[length] = 0x0000;

        if p_lossy {
            todo!("P_LOSSY not implemented utf8 to utf16");
        }

        // TODO: Use uppercase to calculate hash for optimization purposes

        // FUCK DO I KNOW...
        return Ok(nls_string);
    }
}

// TODO: Move somewhere else, anywhere but here
fn to_upper(sb_info: &SbInfo, char16: u16) -> u16 {
    let val = sb_info.upcase_table[char16 as usize];
    if val != 0 {
        return val;
    }
    return char16;
}
