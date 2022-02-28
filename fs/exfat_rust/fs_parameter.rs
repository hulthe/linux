use core::ptr::null;
use kernel::bindings::{fs_param_is_s32, fs_param_is_u32, fs_param_type};
use kernel::c_types;

/// Specification of the type of value a parameter wants.
///
/// (FIXME: copied comment from C)
/// Note that the fsparam_flag(), fsparam_string(), fsparam_u32(), ... methods
/// should be used to generate elements of this type.
#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct FsParameterSpec {
    /// The parameter name
    pub(crate) name: *const c_types::c_char,

    /// The desured parameter type
    pub(crate) type_: fs_param_type,

    /// Option number (returned by fs_parse())
    pub(crate) opt: u8,

    /// TODO
    pub(crate) flags: c_types::c_ushort,

    /// TODO
    pub(crate) data: *const c_types::c_void,
}

unsafe impl Send for FsParameterSpec {}
unsafe impl Sync for FsParameterSpec {}

impl FsParameterSpec {
    pub(crate) const fn fsparam(
        type_: fs_param_type,
        name: &'static [u8],
        opt: ExfatOptions,
        flags: u32,
        data: *const c_types::c_void,
    ) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_,
            opt: opt as u8,
            flags: flags as u16,
            data,
        }
    }

    pub(crate) const fn fsparam_u32(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: Some(fs_param_is_u32),
            opt: opt as u8,
            flags: 0,
            data: null(),
        }
    }

    pub(crate) const fn fsparam_s32(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: Some(fs_param_is_s32),
            opt: opt as u8,
            flags: 0,
            data: null(),
        }
    }

    pub(crate) const fn fsparam_u32oct(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: Some(fs_param_is_s32),
            opt: opt as u8,
            flags: 0,
            data: 8 as *const c_types::c_void,
        }
    }

    pub(crate) const fn fsparam_string(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: None,
            opt: opt as u8,
            flags: 0,
            data: null(),
        }
    }

    pub(crate) const fn fsparam_flag(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: None,
            opt: opt as u8,
            flags: 0,
            data: null(),
        }
    }

    pub(crate) const fn fsparam_enum(
        name: &'static [u8],
        opt: ExfatOptions,
        array: *const c_types::c_void,
    ) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: None,
            opt: opt as u8,
            flags: 0,
            data: array,
        }
    }

    pub(crate) const fn null() -> FsParameterSpec {
        FsParameterSpec {
            name: null(),
            type_: None,
            opt: 0,
            flags: 0,
            data: null(),
        }
    }
}

#[repr(C)]
pub(crate) enum ExfatOptions {
    Uid,
    Gid,
    Umask,
    Dmask,
    Fmask,
    AllowUtime,
    Charset,
    Errors,
    Discard,
    TimeOffset,

    /* Deprecated? */
    Utf8,
    Debug,
    Namecase,
    Codepage,
}
