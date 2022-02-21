//! A Rust implementation of the exFAT filesystem

mod allocation_bitmap;
mod boot_sector;
mod checksum;
mod directory;
mod external;
mod fat;
mod heap;
mod macros;
mod superblock;
mod upcase;
mod inode;

use core::ptr::{null, null_mut};
use kernel::bindings::{
    file_system_type as FileSystemType, fs_context, fs_context_operations as FsContextOps,
    fs_param_deprecated, fs_param_is_s32, fs_param_is_u32, fs_param_type, fs_parameter_spec,
    get_tree_bdev, hlist_head as HlistHead, kill_block_super, lock_class_key as LockClassKey,
    register_filesystem, request_queue as RequestQueue, super_block,
    super_operations as SuperOperations, unregister_filesystem, EXFAT_SUPER_MAGIC, FS_REQUIRES_DEV,
    NSEC_PER_MSEC, QUEUE_FLAG_DISCARD, SB_NODIRATIME,
};
use kernel::c_types;
use kernel::c_types::{c_int, c_void};
use kernel::prelude::*;
use kernel::{Result, pr_warn, ThisModule};
use superblock::{ExfatErrorMode, ExfatMountOptions, SuperBlockInfo};
use inode::InodeHashTable;
use core::pin::Pin;
use kernel::sync::SpinLock;

struct ExFatRust;

/// Table entry to map between kernel "constants" and our "constants"
#[repr(C)]
#[derive(Copy, Clone)]
pub struct ConstantTable {
    /// The kernel's name of the constant
    pub name: *const c_types::c_char,

    /// Our value used to represent the constant
    pub value: c_types::c_int,
}

unsafe impl Send for ConstantTable {}
unsafe impl Sync for ConstantTable {}

/// Specification of the type of value a parameter wants.
///
/// (FIXME: copied comment from C)
/// Note that the fsparam_flag(), fsparam_string(), fsparam_u32(), ... methods
/// should be used to generate elements of this type.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FsParameterSpec {
    /// The parameter name
    pub name: *const c_types::c_char,

    /// The desured parameter type
    pub type_: fs_param_type,

    /// Option number (returned by fs_parse())
    pub opt: u8,

    /// TODO
    pub flags: c_types::c_ushort,

    /// TODO
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

static EXFAT_PARAM_ENUMS: &[ConstantTable] = &[
    ConstantTable {
        name: ExfatErrorMode::Continue.get_name(),
        value: ExfatErrorMode::Continue as i32,
    },
    ConstantTable {
        name: ExfatErrorMode::Panic.get_name(),
        value: ExfatErrorMode::Panic as i32,
    },
    // FIXME: vidde, borde inte detta vara Remount? Vi har Panic på raden över...
    ConstantTable {
        name: ExfatErrorMode::Panic.get_name(),
        value: ExfatErrorMode::Panic as i32,
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
    free: None,                     // TODO
    parse_param: None,              // TODO
    get_tree: Some(exfat_get_tree), // TODO
    reconfigure: None,              // TODO

    // not needed?
    dup: None,
    parse_monolithic: None,
};

extern "C" fn exfat_get_tree(fc: *mut fs_context) -> c_types::c_int {
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

extern "C" fn exfat_fill_super(sb: *mut super_block, _fc: *mut fs_context) -> c_types::c_int {
    from_kernel_result! {
        // Do some things?
        let mut sb = unsafe { *sb };
        let exfat_sb_info: &mut SuperBlockInfo = get_exfat_sb_from_sb!(&mut sb);
        let opts: &mut ExfatMountOptions = &mut exfat_sb_info.options;

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

        read_exfat_partition(&mut sb)?;

        exfat_hash_init(&mut sb);

        Ok(())
    }
}

fn exfat_hash_init(sb: &mut super_block) {
    let sbi: &mut SuperBlockInfo = get_exfat_sb_from_sb!(sb);
    let inode_hash_table: InodeHashTable = InodeHashTable::new();
    // SAFETY: TODO
    let mut inode_hash_table_lock = unsafe { SpinLock::new(inode_hash_table) };
    // SAFETY: TODO
    kernel::spinlock_init!(unsafe { Pin::new_unchecked(&mut inode_hash_table_lock) }, "Exfat inode hashtable spinlock");
    sbi.inode_hashtable = Some(inode_hash_table_lock);
}

fn read_exfat_partition(sb: &mut super_block) -> Result {
    // TODO: Fill in code from __exfat_fill_super.

    // 1. exfat_read_boot_sector
    boot_sector::read_boot_sector(sb)?;

    // 2. exfat_verify_boot_region
    boot_sector::verify_boot_region(sb)?;

    // 3. exfat_create_upcase_table
    upcase::create_upcase_table(sb)?;

    // 4. exfat_load_bitmap
    allocation_bitmap::load_allocation_bitmap(sb)?;

    // 5. exfat_count_used_clusters
    count_used_clusters(sb);

    Ok(())
}

const EXFAT_RESERVED_CLUSTERS: u32 = 2;
const BITS_PER_BYTE_MASK: u32 = 0x7;
const BITS_PER_BYTE: usize = 8;

const USED_BIT: [u8; 256] = [
    0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 1, 2, 2, 3, /*  0 ~  19*/
    2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5, 1, 2, 2, 3, 2, 3, 3, 4, /* 20 ~  39*/
    2, 3, 3, 4, 3, 4, 4, 5, 2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, /* 40 ~  59*/
    4, 5, 5, 6, 1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5, /* 60 ~  79*/
    2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6, 2, 3, 3, 4, /* 80 ~  99*/
    3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6, 3, 4, 4, 5, 4, 5, 5, 6, /*100 ~ 119*/
    4, 5, 5, 6, 5, 6, 6, 7, 1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, /*120 ~ 139*/
    3, 4, 4, 5, 2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6, /*140 ~ 159*/
    2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6, 3, 4, 4, 5, /*160 ~ 179*/
    4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7, 2, 3, 3, 4, 3, 4, 4, 5, /*180 ~ 199*/
    3, 4, 4, 5, 4, 5, 5, 6, 3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, /*200 ~ 219*/
    5, 6, 6, 7, 3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7, /*220 ~ 239*/
    4, 5, 5, 6, 5, 6, 6, 7, 5, 6, 6, 7, 6, 7, 7, 8, /*240 ~ 255*/
];

const LAST_BIT_MASK: [u8; 8] = [
    0, 0b00000001, 0b00000011, 0b00000111, 0b00001111, 0b00011111, 0b00111111, 0b01111111,
];

fn count_used_clusters(sb: &mut super_block) -> u32 {
    let sbi: &mut SuperBlockInfo = get_exfat_sb_from_sb!(sb);
    let total_clus = sbi.boot_sector_info.num_clusters - EXFAT_RESERVED_CLUSTERS;
    let last_mask = total_clus & BITS_PER_BYTE_MASK;

    let total_clus = total_clus & !last_mask;
    let mut map_i = 0;
    let mut map_b = 0;
    let mut clu_bits = 0;
    let mut count = 0;
    for _ in (0..total_clus).step_by(BITS_PER_BYTE) {
        // TODO: Finish when vol_amap is implemented.
        // clu_bits = sbi.vol_amap[map_i].b_data + map_b;

        // Assumes that clu_bits < used_bit length.
        count += USED_BIT[clu_bits] as u32;
        map_b += 1;
        if map_b >= sb.s_blocksize {
            map_i += 1;
            map_b = 0;
        }
    }

    if last_mask != 0 {
        // TODO: Finish when vol_amap is implemented.
        // clu_bits = sbi.vol_amap[map_i].b_data + map_b;

        clu_bits &= LAST_BIT_MASK[last_mask as usize] as usize;
        count += USED_BIT[clu_bits] as u32;
    }

    count
}

/// Initialize ExFat SuperBlockInfo and pass it to fs_context
pub extern "C" fn init_fs_context(fc: *mut fs_context) -> c_int {
    from_kernel_result! {
        pr_info!("init_fs_context called");

        // TODO: properly initialize sb
        // TODO: might overflow the stack
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
