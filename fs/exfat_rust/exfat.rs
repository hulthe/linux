mod allocation_bitmap;
mod boot_sector;
mod charsets;
mod checksum;
mod constant_table;
mod directory;
mod external;
mod fat;
mod file_operations;
mod file_ops;
mod fs_parameter;
mod heap;
mod inode;
mod inode_dir_operations;
mod inode_file_operations;
mod kmem_cache;
mod macros;
mod math;
mod super_operations;
mod superblock;
mod upcase;

use crate::allocation_bitmap::load_allocation_bitmap;
use crate::inode::{InodeExt, INODE_ALLOC_CACHE};
use crate::superblock::{ExfatMountOptions, SbInfo, SbState, SuperBlock, SuperBlockInfo};
use core::mem::MaybeUninit;
use core::pin::Pin;
use core::ptr::null_mut;
use fs_parameter::{exfat_parse_param, EXFAT_PARAMETERS};
use kernel::bindings::{
    __insert_inode_hash, d_make_root, file_system_type as FileSystemType, fs_context as FsContext,
    fs_context_operations as FsContextOps, fs_parameter_spec, get_tree_bdev, inode as Inode,
    inode_set_iversion, kill_block_super, new_inode, register_filesystem,
    request_queue as RequestQueue, unregister_filesystem, CONFIG_EXFAT_DEFAULT_IOCHARSET,
    EXFAT_SUPER_MAGIC, FS_REQUIRES_DEV, NSEC_PER_MSEC, QUEUE_FLAG_DISCARD, SB_NODIRATIME,
};
use kernel::c_types::{c_int, c_void};
use kernel::prelude::*;
use kernel::sync::{Mutex, SpinLock};
use kernel::{pr_warn, Error, Result, ThisModule};

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

    // SAFETY: file comes from C and can be safely zeroed
    ..unsafe { zeroed!(FileSystemType) }
};

pub(crate) extern "C" fn exfat_reconfigure(_fc: *mut FsContext) -> c_int {
    todo!("exfat_reconfigure"); // TODO: implement me
}

pub(crate) extern "C" fn exfat_free(fc: *mut FsContext) {
    // SAFETY: We expect FC to be there (TODO).
    let fc = unsafe { &mut *fc };
    // TODO: unnecessary null check?
    if fc.s_fs_info.is_null() {
        return;
    }

    unsafe {
        // Drop the sbi
        // SAFETY: SuperBlockInfo was allocated by a box
        let sbi = fc.s_fs_info;
        let _ = Box::from_raw(sbi);
    }
}

static mut CONTEXT_OPS: FsContextOps = FsContextOps {
    free: Some(exfat_free),
    parse_param: Some(exfat_parse_param),
    get_tree: Some(exfat_get_tree),
    reconfigure: Some(exfat_reconfigure),

    // SAFETY: file comes from C and can be safely zeroed
    ..unsafe { zeroed!(FsContextOps) }
};

extern "C" fn exfat_get_tree(fc: *mut FsContext) -> c_int {
    // SAFETY: TODO
    return unsafe { get_tree_bdev(fc, Some(exfat_fill_super)) };
}

/* Jan 1 GMT 00:00:00 1980 */
const EXFAT_MIN_TIMESTAMP_SECS: i64 = 315532800;
/* Dec 31 GMT 23:59:59 2107 */
const EXFAT_MAX_TIMESTAMP_SECS: i64 = 4354819199;
const UTF8: &[u8] = b"utf8";
const EXFAT_ROOT_INO: u64 = 1;

extern "C" fn exfat_fill_super(sb: *mut SuperBlock, fc: *mut FsContext) -> c_int {
    pr_info!("exfat_fill_super enter");
    from_kernel_result! {
        // SAFETY: TODO
        let sb = unsafe { &mut *sb };
        let fc = unsafe { &mut *fc };

        pr_info!("fill_super enter");
        let sbi = sb.s_fs_info as *mut MaybeUninit<SuperBlockInfo<'_>>;

        // SAFETY: Value was allocated by a box and so is properly aligned
        // sbi is MaybeUninit so the data is not assumed to be initialized.
        let sbi = unsafe { &mut *sbi };

        let mut options = read_mount_options(fc)?;

        if options.allow_utime == u16::MAX {
            options.allow_utime = !options.fs_dmask & 0022;
        }

        if options.discard {
            let queue: &mut RequestQueue = bdev_get_queue!(&mut sb.s_bdev);

            if (queue.queue_flags >> QUEUE_FLAG_DISCARD) & 1 == 0 {
                // The DISCARD flag is not set for the device
                pr_warn!("mounting with \"discard\" option, but the device does not support discard");
                options.discard = false;
            }
        }

        sb.s_flags |= SB_NODIRATIME as u64;
        sb.s_magic = EXFAT_SUPER_MAGIC as u64;
        sb.s_op = unsafe { &super_operations::EXFAT_SOPS as *const _ };

        sb.s_time_gran = 10 * NSEC_PER_MSEC;
        sb.s_time_min = EXFAT_MIN_TIMESTAMP_SECS;
        sb.s_time_max = EXFAT_MAX_TIMESTAMP_SECS;

        let mut sb_state = SbState { sb };
        let boot_sector_info = boot_sector::read_boot_sector(&mut sb_state)?;
        boot_sector::verify_boot_region(&mut sb_state)?;
        let upcase_table = upcase::create_upcase_table(&boot_sector_info, &mut sb_state)?;

        // Properly initialize sbi and write the struct to the previously allocated memory
        let sbi = sbi.write(SuperBlockInfo {
            allocation_bitmap: load_allocation_bitmap(&boot_sector_info, &mut sb_state)?,

            info: SbInfo {
                boot_sector_info,
                options,
                upcase_table,
            },

            // SAFETY: locks are initialized below
            state: unsafe { Mutex::new(sb_state) },
            inode_hashtable: unsafe { SpinLock::new(Default::default()) },
        });

        // Initialize locks
        kernel::spinlock_init!(
            unsafe { Pin::new_unchecked(&mut sbi.inode_hashtable) },
            "ExFAT inode hashtable spinlock"
        );
        kernel::mutex_init!(
            unsafe { Pin::new_unchecked(&mut sbi.state) },
            "ExFAT superblock mutex"
        );

        let options: &mut ExfatMountOptions = &mut sbi.info.options;
        if &*options.iocharset == UTF8 {
            options.utf8 = true;
        } else {
            // TODO: charset stuff!??!?!
        }

        let sb = &mut sbi.state.get_mut().sb;

        if options.utf8 {
            // TODO
            // sb->s_d_op = &exfat_utf8_dentry_ops;
        } else {
            // TODO
            // sb->s_d_op = &exfat_dentry_ops;
        }

        let root_inode: &mut Inode = unsafe { new_inode(*sb).as_mut() }.ok_or_else(|| {
            pr_err!("Failed to allocate root inode");
            Error::ENOMEM
        })?;

        root_inode.i_ino = EXFAT_ROOT_INO;
        // SAFETY: TODO
        unsafe { inode_set_iversion(root_inode, 1) };
        inode::read_root_inode(root_inode, sbi).map_err(|e| {
            pr_err!("failed to initialize root inode, err: {:?}", e);
            e
        })?;

        sbi.inode_hashtable.lock().insert(root_inode.to_info_mut());
        unsafe { __insert_inode_hash(root_inode, root_inode.to_info().unique_num()) };

        let sb = &mut sbi.state.get_mut().sb;
        // SAFETY: TODO: The kernel giveth, the kernel taketh away
        sb.s_root = unsafe { d_make_root(root_inode) };
        if sb.s_root.is_null() {
            pr_err!("failed to get the root dentry");
            return Err(Error::ENOMEM);
        }

        pr_info!("exfat_fill_super exit");

        Ok(())
    }
}

fn read_mount_options(_fc: &mut FsContext) -> Result<ExfatMountOptions> {
    Ok(ExfatMountOptions {
        fs_uid: Default::default(),   // TODO
        fs_gid: Default::default(),   // TODO
        fs_fmask: Default::default(), // TODO: current->fs->umask,
        fs_dmask: Default::default(), // TODO: current->fs->umask,
        allow_utime: u16::MAX,        // TODO
        iocharset: CONFIG_EXFAT_DEFAULT_IOCHARSET
            .try_to_vec()?
            .try_into_boxed_slice()?,
        errors: superblock::ExfatErrorMode::RemountRo,
        utf8: true,                      // TODO
        discard: Default::default(),     // TODO
        time_offset: Default::default(), // TODO
    })
}

const BITS_PER_BYTE: usize = 8;

/// Allocate ExFat SuperBlockInfo and pass it to fc
pub extern "C" fn init_fs_context(fc: *mut FsContext) -> c_int {
    from_kernel_result! {
        pr_info!("init_fs_context enter");

        let fc = unsafe { &mut *fc };

        // sbi is lazily initialized in fill_super
        let sbi: Box<MaybeUninit<SuperBlockInfo<'_>>> = Box::try_new(MaybeUninit::zeroed())?;
        let sbi = Box::into_raw(sbi);

        // SAFETY: TODO
        fc.s_fs_info = sbi as *mut c_void;

        // SAFETY: TODO
        fc.ops = unsafe { &CONTEXT_OPS as *const _ };

        pr_info!("init_fs_context exit");
        Ok(())
    }
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
        unsafe { FS_TYPE.owner = module.0 };

        // bindgen is having trouble with these constants. TODO: move them somewhere else
        const SLAB_RECLAIM_ACCOUNT: u32 = 0x00020000;
        const SLAB_MEM_SPREAD: u32 = 0x0010000;
        if let Err(e) = unsafe {
            INODE_ALLOC_CACHE.create(
                "exfat inode cache\0",
                SLAB_RECLAIM_ACCOUNT | SLAB_MEM_SPREAD,
            )
        } {
            pr_err!("failed to initialize inode cache: {e:?}");
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
