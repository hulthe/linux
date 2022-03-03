//! A Rust implementation of the exFAT filesystem
mod allocation_bitmap;
mod boot_sector;
mod checksum;
mod constant_table;
mod directory;
mod external;
mod fat;
mod file_ops;
mod fs_parameter;
mod heap;
mod inode;
mod inode_dir_operations;
mod macros;
mod math;
mod superblock;
mod upcase;

use core::pin::Pin;
use core::ptr::null_mut;
use fs_parameter::{exfat_parse_param, EXFAT_PARAMETERS};
use kernel::bindings::{
    d_make_root, file_system_type as FileSystemType, fs_context,
    fs_context_operations as FsContextOps, fs_parameter_spec, get_tree_bdev,
    hlist_head as HlistHead, inode as Inode, inode_set_iversion, kill_block_super,
    lock_class_key as LockClassKey, new_inode, register_filesystem, request_queue as RequestQueue,
    super_block, super_operations as SuperOperations, unregister_filesystem, EXFAT_SUPER_MAGIC,
    FS_REQUIRES_DEV, NSEC_PER_MSEC, QUEUE_FLAG_DISCARD, SB_NODIRATIME,
};
use kernel::c_types;
use kernel::c_types::{c_int, c_void};
use kernel::prelude::*;
use kernel::sync::{Mutex, SpinLock};
use kernel::{pr_warn, Error, Result, ThisModule};
use superblock::{ExfatMountOptions, SbState, SuperBlock, SuperBlockInfo};

struct ExFatRust;

static mut FS_TYPE: FileSystemType = FileSystemType {
    // seems to be required
    name: b"exfat_rust\0".as_ptr() as *const i8,
    owner: null_mut(), // should be THIS_MODULE, is set by module init
    init_fs_context: Some(init_fs_context),
    parameters: EXFAT_PARAMETERS as *const _ as *const fs_parameter_spec,
    // TODO: we should make sure the destructor for SuperBlockInfo is run
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
    free: None, // TODO
    parse_param: Some(exfat_parse_param),
    get_tree: Some(exfat_get_tree),
    reconfigure: None, // TODO

    // not needed?
    dup: None,
    parse_monolithic: None,
};

extern "C" fn exfat_get_tree(fc: *mut fs_context) -> c_types::c_int {
    // SAFETY: TODO
    return unsafe { get_tree_bdev(fc, Some(exfat_fill_super)) };
}

static mut EXFAT_SOPS: SuperOperations = SuperOperations {
    alloc_inode: None,  // TODO
    free_inode: None,   // TODO
    write_inode: None,  // TODO
    evict_inode: None,  // TODO
    put_super: None,    // TODO
    sync_fs: None,      // TODO
    statfs: None,       // TODO
    show_options: None, // TODO

    // Not implemented in C version
    destroy_inode: None,
    dirty_inode: None,
    drop_inode: None,
    freeze_super: None,
    freeze_fs: None,
    thaw_super: None,
    unfreeze_fs: None,
    remount_fs: None,
    umount_begin: None,
    show_devname: None,
    show_path: None,
    show_stats: None,
    quota_read: None,
    quota_write: None,
    get_dquots: None,
    nr_cached_objects: None,
    free_cached_objects: None,
};

/* Jan 1 GMT 00:00:00 1980 */
const EXFAT_MIN_TIMESTAMP_SECS: i64 = 315532800;
/* Dec 31 GMT 23:59:59 2107 */
const EXFAT_MAX_TIMESTAMP_SECS: i64 = 4354819199;
const UTF8: &str = "utf8";
const EXFAT_ROOT_INO: u64 = 1;

extern "C" fn exfat_fill_super(sb: *mut super_block, _fc: *mut fs_context) -> c_types::c_int {
    from_kernel_result! {
        // SAFETY: TODO
        let sb = unsafe { &mut *sb };
        fill_super(sb)?;
        Ok(())
    }
}

fn fill_super(sb: &mut SuperBlock) -> Result {
    pr_info!("exfat_fill_super enter");
    let exfat_sb_info = get_exfat_sb_from_sb!(sb);
    //let exfat_sb_info = Pin::new(exfat_sb_info);
    //let exfat_sb_info: &mut SuperBlockInfo = exfat_sb_info.get_mut();

    let opts: &mut ExfatMountOptions = &mut exfat_sb_info.info.options;
    if opts.allow_utime == u16::MAX {
        opts.allow_utime = !opts.fs_dmask & 0022;
    }

    if opts.discard {
        let queue: &mut RequestQueue = bdev_get_queue!(&mut sb.s_bdev);

        if (queue.queue_flags >> QUEUE_FLAG_DISCARD) & 1 == 0 {
            // The DISCARD flag is not set for the device
            pr_warn!("mounting with \"discard\" option, but the device does not support discard");
            opts.discard = false;
        }
    }

    sb.s_flags |= SB_NODIRATIME as u64;
    sb.s_magic = EXFAT_SUPER_MAGIC as u64;
    sb.s_op = unsafe { &EXFAT_SOPS as *const _ };

    sb.s_time_gran = 10 * NSEC_PER_MSEC;
    sb.s_time_min = EXFAT_MIN_TIMESTAMP_SECS;
    sb.s_time_max = EXFAT_MAX_TIMESTAMP_SECS;

    let sb_state = unsafe { Mutex::new(SbState { sb }) };
    exfat_sb_info.state = Some(sb_state);
    kernel::mutex_init!(
        unsafe { Pin::new_unchecked(exfat_sb_info.state.as_mut().unwrap()) },
        "ExFAT superblock mutex"
    );

    read_exfat_partition(exfat_sb_info)?;

    exfat_hash_init(exfat_sb_info);

    let opts: &mut ExfatMountOptions = &mut exfat_sb_info.info.options;
    if opts.iocharset != UTF8 {
        opts.utf8 = true;
    } else {
        // TODO: charset stuff!??!?!
    }

    // TODO: Finished function
    let sb = &mut exfat_sb_info.state.as_mut().unwrap().get_mut().sb;
    let root_inode: &mut Inode = unsafe { new_inode(*sb).as_mut() }.ok_or_else(|| {
        pr_err!("Failed to allocate root inode");
        Error::ENOMEM
    })?;
    root_inode.i_ino = EXFAT_ROOT_INO;
    // SAFETY: TODO
    unsafe {
        inode_set_iversion(root_inode, 1);
    }
    inode::read_root_inode(root_inode, exfat_sb_info)?;

    let sb = &mut exfat_sb_info.state.as_mut().unwrap().get_mut().sb;
    // SAFETY: TODO: The kernel giveth, the kernel taketh away
    sb.s_root = unsafe { d_make_root(root_inode) };

    pr_info!("exfat_fill_super exit");

    Ok(())
}

fn exfat_hash_init(sbi: &mut SuperBlockInfo<'_>) {
    // SAFETY: TODO
    kernel::spinlock_init!(
        unsafe { Pin::new_unchecked(&mut sbi.inode_hashtable) },
        "ExFAT inode hashtable spinlock"
    );
}

fn read_exfat_partition(sbi: &mut SuperBlockInfo<'_>) -> Result {
    // TODO: Add logging on returns

    // 1. exfat_read_boot_sector
    boot_sector::read_boot_sector(sbi)?;

    // 2. exfat_verify_boot_region
    boot_sector::verify_boot_region(sbi)?;

    // 3. exfat_create_upcase_table
    upcase::create_upcase_table(sbi)?;

    // 4. exfat_load_bitmap
    allocation_bitmap::load_allocation_bitmap(sbi)?;

    Ok(())
}

const BITS_PER_BYTE: usize = 8;

/// Initialize ExFat SuperBlockInfo and pass it to fs_context
pub extern "C" fn init_fs_context(fc: *mut fs_context) -> c_int {
    from_kernel_result! {
        pr_info!("init_fs_context enter");

        // TODO: properly initialize sb
        // TODO: might overflow the stack
        let sbi = Box::try_new(SuperBlockInfo {
            info: Default::default(),

            allocation_bitmap: Default::default(),

            state: None,
            inode_hashtable: unsafe { SpinLock::new(Default::default()) },
        })?;

        // SAFETY: TODO
        let fc = unsafe { &mut *fc };
        fc.s_fs_info = Box::into_raw(sbi) as *mut c_void;

        // SAFETY: TODO
        fc.ops = unsafe { &CONTEXT_OPS as *const _ };


        pr_info!("init_fs_context exit");
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

        // SAFETY: TODO
        unsafe {
            FS_TYPE.owner = module.0;
        }

        // SAFETY: TODO
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

        // SAFETY: TODO
        let err = unsafe { unregister_filesystem(&mut FS_TYPE as *mut _) };
        if err != 0 {
            pr_info!(
                "### Rust ExFat ### error unregistering file system: {} \n",
                err
            );
        }
    }
}
