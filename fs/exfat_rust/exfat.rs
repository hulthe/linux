//! A Rust implementation of the exFAT filesystem

mod superblock;

use core::ptr::{null, null_mut};
use kernel::bindings::{
    constant_table, file_system_type as FileSystemType, fs_context,
    fs_context_operations as FsContextOps, fs_param_deprecated, fs_param_is_enum, fs_param_is_s32,
    fs_param_is_string, fs_param_is_u32, fs_param_type, hlist_head as HlistHead, kill_block_super,
    lock_class_key as LockClassKey, register_filesystem, unregister_filesystem, FS_REQUIRES_DEV,
    fs_parameter_spec,
};
use kernel::c_types;
use kernel::c_types::{c_int, c_void};
use kernel::prelude::*;
use kernel::Result;
use kernel::ThisModule;
use superblock::SuperBlockInfo;

struct ExFatRust;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ConstantTable {
    pub name: *const c_types::c_char,
    pub value: c_types::c_int,
}

unsafe impl Send for ConstantTable {}
unsafe impl Sync for ConstantTable {}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FsParameterSpec {
    pub name: *const c_types::c_char,
    pub type_: fs_param_type,
    pub opt: u8,
    pub flags: c_types::c_ushort,
    pub data: *const c_types::c_void,
}

unsafe impl Send for FsParameterSpec {}
unsafe impl Sync for FsParameterSpec {}

impl FsParameterSpec {
    const fn fsparam(
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

    const fn fsparam_u32(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: Some(fs_param_is_u32),
            opt: opt as u8,
            flags: 0,
            data: null(),
        }
    }

    const fn fsparam_s32(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: Some(fs_param_is_s32),
            opt: opt as u8,
            flags: 0,
            data: null(),
        }
    }

    const fn fsparam_u32oct(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: Some(fs_param_is_s32),
            opt: opt as u8,
            flags: 0,
            data: 8 as *const c_types::c_void,
        }
    }

    const fn fsparam_string(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: None,
            opt: opt as u8,
            flags: 0,
            data: null(),
        }
    }

    const fn fsparam_flag(name: &'static [u8], opt: ExfatOptions) -> FsParameterSpec {
        FsParameterSpec {
            name: name.as_ptr() as *const i8,
            type_: None,
            opt: opt as u8,
            flags: 0,
            data: null(),
        }
    }

    const fn fsparam_enum(
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

    const fn null() -> FsParameterSpec {
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
enum ExfatErrorMode {
    EXFAT_ERRORS_CONT,
    EXFAT_ERRORS_PANIC,
    EXFAT_ERRORS_RO,
}

impl ExfatErrorMode {
    const fn get_name(self) -> *const c_types::c_char {
        match self {
            ExfatErrorMode::EXFAT_ERRORS_CONT => b"continue\0".as_ptr() as *const i8,
            ExfatErrorMode::EXFAT_ERRORS_PANIC => b"panic\0".as_ptr() as *const i8,
            ExfatErrorMode::EXFAT_ERRORS_RO => b"remount-ro\0".as_ptr() as *const i8,
        }
    }
}

static EXFAT_PARAM_ENUMS: &[ConstantTable] = &[
    ConstantTable {
        name: ExfatErrorMode::EXFAT_ERRORS_CONT.get_name(),
        value: ExfatErrorMode::EXFAT_ERRORS_CONT as i32,
    },
    ConstantTable {
        name: ExfatErrorMode::EXFAT_ERRORS_PANIC.get_name(),
        value: ExfatErrorMode::EXFAT_ERRORS_PANIC as i32,
    },
    ConstantTable {
        name: ExfatErrorMode::EXFAT_ERRORS_PANIC.get_name(),
        value: ExfatErrorMode::EXFAT_ERRORS_PANIC as i32,
    },
    // Null terminator?
    ConstantTable {
        name: null(),
        value: 0,
    },
];

#[repr(C)]
enum ExfatOptions {
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

static EXFAT_PARAMETERS: &[FsParameterSpec] = &[
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

static mut FS_TYPE: FileSystemType = FileSystemType {
    // seems to be required
    name: b"exfat_rust\0".as_ptr() as *const i8,
    owner: null_mut(), // should be THIS_MODULE, is set by module init
    init_fs_context: Some(init_fs_context),
    parameters: EXFAT_PARAMETERS as *const _ as *const fs_parameter_spec,
    kill_sb: Some(kill_block_super),
    fs_flags: FS_REQUIRES_DEV as i32,

    // seems we can leave these as default
    mount: None,
    next: null_mut(),
    fs_supers: empty_hlist(),
    s_lock_key: LockClassKey {},
    s_umount_key: LockClassKey {},
    s_vfs_rename_key: LockClassKey {},
    s_writers_key: [LockClassKey {}; 3],
    i_lock_key: LockClassKey {},
    i_mutex_key: LockClassKey {},
    invalidate_lock_key: LockClassKey {},
    i_mutex_dir_key: LockClassKey {},
};

static mut CONTEXT_OPS: FsContextOps = FsContextOps {
    free: None,        // TODO
    parse_param: None, // TODO
    get_tree: None,    // TODO
    reconfigure: None, // TODO

    // not needed?
    dup: None,
    parse_monolithic: None,
};

macro_rules! from_kernel_result {
    ($($tt:tt)*) => {{
        match (|| {
            $($tt)*
        })() {
            kernel::Result::Ok(()) => 0,
            kernel::Result::Err(e) => e.to_kernel_errno(),
        }
    }};
}

pub extern "C" fn init_fs_context(fc: *mut fs_context) -> c_int {
    from_kernel_result! {
        pr_info!("init_fs_context called");

        // TODO: properly initialize sb
        let sbi = Box::try_new(SuperBlockInfo::default())?;

        let fc = unsafe { &mut *fc };
        fc.s_fs_info = Box::into_raw(sbi) as *mut c_void;


        fc.ops = unsafe { &CONTEXT_OPS as *const _ };


        Ok(())
    }
}

const fn empty_hlist() -> HlistHead {
    HlistHead { first: null_mut() }
}

module! {
    type: ExFatRust,
    name: b"exfat_rust",
    author: b"Rust for Linux Contributors",
    description: b"ExFat in Rust",
    license: b"GPL v2",
}

impl KernelModule for ExFatRust {
    fn init(_name: &'static CStr, module: &'static ThisModule) -> Result<Self> {
        pr_info!("### Rust ExFat ### init\n");

        unsafe {
            FS_TYPE.owner = module.0;
        }

        let err = unsafe { register_filesystem(&mut FS_TYPE as *mut _) };

        if err != 0 {
            pr_info!(
                "### Rust ExFat ### error registering file system: {} \n",
                err
            );
        }
        Ok(ExFatRust)
    }
}

impl Drop for ExFatRust {
    fn drop(&mut self) {
        pr_info!("### Rust ExFat ### exit\n");

        let err = unsafe { unregister_filesystem(&mut FS_TYPE as *mut _) };
        if err != 0 {
            pr_info!(
                "### Rust ExFat ### error unregistering file system: {} \n",
                err
            );
        }
    }
}
