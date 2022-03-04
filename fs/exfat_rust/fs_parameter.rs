use crate::constant_table::ConstantTable;
use crate::get_exfat_sb_from_fc;
use crate::superblock::{ExfatErrorMode, SuperBlockInfo};
use alloc::boxed::Box;
use core::mem::MaybeUninit;
use core::ptr::{null, null_mut};
use kernel::bindings::{
    current_user_ns, fs_context as FsContext, fs_param_deprecated, fs_param_is_s32,
    fs_param_is_u32, fs_param_type, fs_parameter as FsParameter, fs_parameter_spec, fs_parse,
    fs_parse_result as FsParseResult, make_kgid, make_kuid,
};
use kernel::str::CStr;
use kernel::{c_types, Error, Result};

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

impl ExfatOptions {
    fn from_c_int(num: c_types::c_int) -> Result<Self> {
        Ok(match num {
            0 => Self::Uid,
            1 => Self::Gid,
            2 => Self::Umask,
            3 => Self::Dmask,
            4 => Self::Fmask,
            5 => Self::AllowUtime,
            6 => Self::Charset,
            7 => Self::Errors,
            8 => Self::Discard,
            9 => Self::TimeOffset,
            10 => Self::Utf8,
            11 => Self::Debug,
            12 => Self::Namecase,
            13 => Self::Codepage,
            // TODO: This might cause a problem as the C code doesn't do it this way.
            _ => return Err(Error::EINVAL),
        })
    }
}

static EXFAT_PARAM_ENUMS: &[ConstantTable] = &[
    ConstantTable {
        name: ExfatErrorMode::Continue.get_name(),
        value: ExfatErrorMode::Continue as i32,
    },
    ConstantTable {
        name: ExfatErrorMode::Panic.get_name(),
        value: ExfatErrorMode::Panic as i32,
    },
    ConstantTable {
        name: ExfatErrorMode::RemountRo.get_name(),
        value: ExfatErrorMode::RemountRo as i32,
    },
    // Null terminator?
    ConstantTable {
        name: null(),
        value: 0,
    },
];

pub(crate) static EXFAT_PARAMETERS: &[FsParameterSpec] = &[
    FsParameterSpec::fsparam_u32(b"uid\0", ExfatOptions::Uid),
    FsParameterSpec::fsparam_u32(b"gid\0", ExfatOptions::Gid),
    FsParameterSpec::fsparam_u32oct(b"umask\0", ExfatOptions::Umask),
    FsParameterSpec::fsparam_u32oct(b"dmask\0", ExfatOptions::Dmask),
    FsParameterSpec::fsparam_u32oct(b"fmask\0", ExfatOptions::Fmask),
    FsParameterSpec::fsparam_u32oct(b"allow_utime\0", ExfatOptions::AllowUtime),
    FsParameterSpec::fsparam_string(b"iocharset\0", ExfatOptions::Charset),
    FsParameterSpec::fsparam_enum(
        b"errors\0",
        ExfatOptions::Errors,
        EXFAT_PARAM_ENUMS as *const _ as *const c_types::c_void,
    ),
    FsParameterSpec::fsparam_flag(b"discard\0", ExfatOptions::Discard),
    FsParameterSpec::fsparam_s32(b"time_offset\0", ExfatOptions::TimeOffset),
    FsParameterSpec::fsparam(
        None,
        b"utf8\0",
        ExfatOptions::Utf8,
        fs_param_deprecated,
        null(),
    ),
    FsParameterSpec::fsparam(
        None,
        b"debug\0",
        ExfatOptions::Debug,
        fs_param_deprecated,
        null(),
    ),
    FsParameterSpec::fsparam(
        None,
        b"namecase\0",
        ExfatOptions::Namecase,
        fs_param_deprecated,
        null(),
    ),
    FsParameterSpec::fsparam(
        None,
        b"codepage\0",
        ExfatOptions::Codepage,
        fs_param_deprecated,
        null(),
    ),
    // Null terminator?
    FsParameterSpec::null(),
];

pub(crate) extern "C" fn exfat_parse_param(
    fc: *mut FsContext,
    parameter: *mut FsParameter,
) -> c_types::c_int {
    // SAFETY: TODO
    let fc = unsafe { &mut *fc };
    // SAFETY: TODO
    let parameter = unsafe { &mut *parameter };
    match parse_param(fc, parameter) {
        Ok(errno) => errno,
        Err(e) => e.to_kernel_errno(),
    }
}

// TODO: Don't know what this is, exists in the C code.
const UTIME_MASK: u16 = 0o22;

// TODO: This method returns a result of an i32 where the i32 represents a kernel error number (or 0 if everything went ok).
// This is done because from_kernel_errno is not public in the kernel crate.
fn parse_param(fc: &mut FsContext, parameter: &mut FsParameter) -> Result<i32> {
    let sbi: &mut SuperBlockInfo<'_> = get_exfat_sb_from_fc!(fc);

    let mut parse_result: MaybeUninit<FsParseResult> = MaybeUninit::uninit();
    // SAFETY: TODO
    let opt_res = unsafe {
        fs_parse(
            fc,
            EXFAT_PARAMETERS as *const _ as *const fs_parameter_spec,
            parameter,
            parse_result.as_mut_ptr() as *mut _ as *mut FsParseResult,
        )
    };

    if opt_res < 0 {
        // TODO: Should return opt but from_kernel_errno is not public
        return Ok(opt_res);
        // return Err(Error::EINVAL);
    }

    // SAFETY: Since opt_res was not an error, parse_result should have been initialized
    let parse_result = unsafe { parse_result.assume_init() };

    let opt: ExfatOptions = ExfatOptions::from_c_int(opt_res)?;

    // TODO: Finish
    match opt {
        ExfatOptions::Uid => {
            sbi.info.options.fs_uid =
            // SAFETY: Due to opt being Uid, the result should be a uint_32
                unsafe { make_kuid(current_user_ns(), parse_result.__bindgen_anon_1.uint_32) }
        }
        ExfatOptions::Gid => {
            sbi.info.options.fs_gid =
            // SAFETY: Due to opt being Gid, the result should be a uint_32
                unsafe { make_kgid(current_user_ns(), parse_result.__bindgen_anon_1.uint_32) }
        }
        ExfatOptions::Umask => {
            // SAFETY: Due to opt being Umask, the result should be a uint_32
            sbi.info.options.fs_fmask =
                unsafe { parse_result.__bindgen_anon_1.uint_32 } as c_types::c_ushort;
            // SAFETY: Due to opt being Umask, the result should be a uint_32
            sbi.info.options.fs_dmask =
                unsafe { parse_result.__bindgen_anon_1.uint_32 } as c_types::c_ushort;
        }
        ExfatOptions::Dmask => {
            // SAFETY: Due to opt being Dmask, the result should be a uint_32
            sbi.info.options.fs_dmask =
                unsafe { parse_result.__bindgen_anon_1.uint_32 } as c_types::c_ushort;
        }
        ExfatOptions::Fmask => {
            // SAFETY: Due to opt being Fmask, the result should be a uint_32
            sbi.info.options.fs_fmask =
                unsafe { parse_result.__bindgen_anon_1.uint_32 } as c_types::c_ushort;
        }
        ExfatOptions::AllowUtime => {
            // SAFETY: Due to opt being AllowUtime, the result should be a uint_32
            sbi.info.options.allow_utime = (unsafe { parse_result.__bindgen_anon_1.uint_32 }
                as c_types::c_ushort)
                & UTIME_MASK;
        }
        ExfatOptions::Charset => {
            // TODO: C code calls exfat_free_iocharset here, should we also do that?
            // SAFETY: Due to opt being Charset, the result should be a string
            sbi.info.options.iocharset = unsafe {
                Box::from_raw(
                    CStr::from_char_ptr(parameter.__bindgen_anon_1.string) as *const _ as *mut _,
                )
            };
            // unsafe { Box::from_raw(CStr::from_char_ptr(parameter.__bindgen_anon_1.string)) };
            parameter.__bindgen_anon_1.string = null_mut();
        }
        ExfatOptions::Errors => {
            // SAFETY: Due to opt being Errors, the result should be a uint_32
            sbi.info.options.errors =
                ExfatErrorMode::from_c_int(unsafe { parse_result.__bindgen_anon_1.uint_32 })?;
        }
        ExfatOptions::TimeOffset => {
            // Kept comment from C code:
            // Make the limit 24 just in case someone invents something unusual.
            // SAFETY: Due to opt being TimeOFfset, the result should be a int_32
            if unsafe { parse_result.__bindgen_anon_1.int_32 } < -24 * 60
                || unsafe { parse_result.__bindgen_anon_1.int_32 } > 24 * 60
            {
                return Err(Error::EINVAL);
            }

            // SAFETY: Due to opt being TimeOffset, the result should be a int_32
            sbi.info.options.time_offset = unsafe { parse_result.__bindgen_anon_1.int_32 };
        }
        // We return EINVAL on parsing so we know that 'opt' is one of the valid values.
        _ => {}
    }

    Ok(0)
}
