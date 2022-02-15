//! A Rust implementation of the exFAT filesystem

mod superblock;

use core::ptr::{null, null_mut};
use kernel::bindings::{
    file_system_type as FileSystemType, fs_context, fs_context_operations as FsContextOps,
    hlist_head as HlistHead, kill_block_super, lock_class_key as LockClassKey, register_filesystem,
    unregister_filesystem, FS_REQUIRES_DEV,
};
use kernel::c_types::{c_int, c_void};
use kernel::prelude::*;
use kernel::Result;
use kernel::ThisModule;
use superblock::SuperBlockInfo;

struct ExFatRust;

static mut FS_TYPE: FileSystemType = FileSystemType {
    // seems to be required
    name: b"exfat_rust\0".as_ptr() as *const i8,
    owner: null_mut(), // should be THIS_MODULE, is set by module init
    init_fs_context: Some(init_fs_context),
    parameters: null(), // TODO: should be exfat_parameters
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
