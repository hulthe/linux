//! A Rust implementation of the exFAT filesystem

#![no_std]

use core::ptr::{null, null_mut};
use kernel::bindings::{
    file_system_type, hlist_head, lock_class_key, register_filesystem, unregister_filesystem,
    FS_REQUIRES_DEV,
};
use kernel::prelude::*;
use kernel::ThisModule;

struct ExFatRust;

static mut FS_TYPE: file_system_type = file_system_type {
    // seems to be required
    name: b"exfat_rust\0".as_ptr() as *const i8,
    owner: null_mut(),     // TODO: should be THIS_MODULE
    init_fs_context: None, // TODO: should be exfat_init_fs_context
    parameters: null(),    // TODO: should be exfat_parameters
    kill_sb: None,         // TODO: should be kill_block_super
    fs_flags: FS_REQUIRES_DEV as i32,

    // seems we can leave these as default
    mount: None,
    next: null_mut(),
    fs_supers: empty_hlist(),
    s_lock_key: lock_class_key {},
    s_umount_key: lock_class_key {},
    s_vfs_rename_key: lock_class_key {},
    s_writers_key: [lock_class_key {}; 3],
    i_lock_key: lock_class_key {},
    i_mutex_key: lock_class_key {},
    invalidate_lock_key: lock_class_key {},
    i_mutex_dir_key: lock_class_key {},
};

const fn empty_hlist() -> hlist_head {
    hlist_head { first: null_mut() }
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
