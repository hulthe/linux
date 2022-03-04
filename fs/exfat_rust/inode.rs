use crate::directory::file::{FileAttributes, ROOT_FILE_ATTRIBUTE};
use crate::directory::{ExfatDirEntry, ExfatDirEntryReader};
use crate::fat::FatChainReader;
use crate::file_ops::DIR_OPERATIONS;
use crate::inode_dir_operations::DIR_INODE_OPERATIONS;
use crate::kmem_cache::KMemCache;
use crate::kmem_cache::PtrInit;
use crate::math;
use crate::superblock::{SuperBlock, SuperBlockInfo};
use alloc::vec::Vec;
use core::mem::align_of;
use core::ptr::{null_mut, NonNull};
use kernel::bindings::{
    current_time, i_size_read, i_size_write, inode_inc_iversion, inode_init_once, set_nlink,
    ___GFP_DIRECT_RECLAIM, ___GFP_IO, ___GFP_KSWAPD_RECLAIM,
};
use kernel::Result;

pub(crate) type Inode = kernel::bindings::inode;

// TODO: consider making this not a global. e.g. by putting it in the superblock
pub(crate) static INODE_CACHE: KMemCache<InodeInfo> = KMemCache::new();

const EXFAT_HASH_BITS: usize = 8;
const EXFAT_HASH_SIZE: usize = 1 << EXFAT_HASH_BITS;

#[allow(dead_code)]
pub(crate) struct InodeHashTable {
    inner: [Vec<Inode>; EXFAT_HASH_SIZE],
}

impl Default for InodeHashTable {
    fn default() -> Self {
        const EMPTY: Vec<Inode> = Vec::new();
        Self {
            inner: [EMPTY; EXFAT_HASH_SIZE],
        }
    }
}

#[repr(C)]
pub(crate) struct InodeInfo {
    // SAFETY: vfs_inode MUST BE at the top of this struct,
    // otherwise hell will break lose and the angry angry
    // memory gods will forever be your nemesis.
    // DO NOT TOUCH!!!!!!!! (:cry:)!!!!!!!
    pub(crate) vfs_inode: Inode,
    // struct exfat_chain dir;
    pub(crate) entry: u32,
    pub(crate) start_cluster: u32,
    // unsigned int type;
    // unsigned short attr;
    // unsigned char flags;
    // /*
    //  * the copy of low 32bit of i_version to check
    //  * the validation of hint_stat.
    //  */
    // unsigned int version;
    //
    // /* hint for cluster last accessed */
    // struct exfat_hint hint_bmap;
    // /* hint for entry index we try to lookup next time */
    // struct exfat_hint hint_stat;
    // /* hint for first empty entry */
    // struct exfat_hint_femp hint_femp;
    //
    // spinlock_t cache_lru_lock;
    // struct list_head cache_lru;
    // int nr_caches;
    // /* for avoiding the race between alloc and free */
    // unsigned int cache_valid_id;
    //
    // /*
    //  * NOTE: i_size_ondisk is 64bits, so must hold ->inode_lock to access.
    //  * physically allocated size.
    //  */
    // loff_t i_size_ondisk;
    // /* block-aligned i_size (used in cont_write_begin) */
    // loff_t i_size_aligned;
    // /* on-disk position of directory entry or 0 */
    // loff_t i_pos;
    // /* hash by i_location */
    // struct hlist_node i_hash_fat;
    // /* protect bmap against truncate */
    // struct rw_semaphore truncate_lock;
    // struct inode vfs_inode;
    // /* File creation time */
    // struct timespec64 i_crtime;
}

impl PtrInit for InodeInfo {
    fn init_ptr(ptr: NonNull<Self>) {
        assert_eq!(
            align_of::<Inode>(),
            align_of::<InodeInfo>(),
            "cast Inode to InodeInfo"
        );

        let kernel_inode_ptr: NonNull<Inode> = ptr.cast();
        unsafe { inode_init_once(kernel_inode_ptr.as_ptr()) };
        let kernel_inode = unsafe { *kernel_inode_ptr.as_ptr() };

        let inode = InodeInfo {
            vfs_inode: kernel_inode,

            // zero-init everything
            entry: 0,
            start_cluster: 0,
        };

        unsafe { ptr.as_ptr().write(inode) };
    }
}

pub(crate) trait InodeExt {
    fn to_info(&self) -> &InodeInfo;
    fn to_info_mut(&mut self) -> &mut InodeInfo;
}

impl InodeExt for Inode {
    fn to_info(&self) -> &InodeInfo {
        let inode_info = self as *const _ as *const InodeInfo;
        // SAFETY: TODO
        unsafe { &*inode_info }
    }

    fn to_info_mut(&mut self) -> &mut InodeInfo {
        let inode_info = self as *mut _ as *mut InodeInfo;
        // SAFETY: TODO
        unsafe { &mut *inode_info }
    }
}

// Representing `.` and `..`?
const EXFAT_MIN_SUBDIR: u32 = 2;

pub(crate) extern "C" fn alloc_inode(_sb: *mut SuperBlock) -> *mut Inode {
    kernel::pr_info!("alloc_inode called");
    // bindgen is confused by these constants. // TODO move them
    const __GFP_RECLAIM: u32 = ___GFP_DIRECT_RECLAIM | ___GFP_KSWAPD_RECLAIM;
    const GFP_NOFS: u32 = __GFP_RECLAIM | ___GFP_IO;
    if let Ok(ei) = INODE_CACHE.alloc(GFP_NOFS) {
        // TODO: initialize locks
        unsafe { &mut (*ei.as_ptr()).vfs_inode }
    } else {
        null_mut()
    }
}

// C name `exfat_read_root`
pub(crate) fn read_root_inode(inode: &mut Inode, sbi: &mut SuperBlockInfo<'_>) -> Result {
    // TODO: We probably want to use this for something :shrug:
    let info: &mut InodeInfo = inode.to_info_mut();
    let inode = &mut info.vfs_inode;

    let sb_info = &sbi.info;
    let sb_state = sbi.state.as_mut().unwrap().get_mut();
    let sb = &mut sb_state.sb;

    let root_dir = sb_info.boot_sector_info.root_dir;
    info.start_cluster = root_dir;
    let chain_reader = FatChainReader::new(sb, root_dir);

    fn count_oks<T>(bucket: Result<u32>, item: Result<T>) -> Result<u32> {
        let _ = item?;
        Ok(bucket? + 1)
    }

    let num_clusters = chain_reader.fold(Ok(0), count_oks)?;

    let clusters_size = (num_clusters << sbi.info.boot_sector_info.cluster_size_bits) as i64;
    // SAFETY: TODO
    unsafe {
        i_size_write(inode, clusters_size);
    }

    let dir_reader = ExfatDirEntryReader::new(sb_info, sb_state, root_dir)?;
    let num_subdirs = dir_reader
        .filter_map(|dir_entry| match dir_entry {
            Err(e) => Some(Err(e)),
            Ok(ExfatDirEntry::File(file)) => {
                if file.file_attributes.directory() {
                    Some(Ok(()))
                } else {
                    None
                }
            }
            _ => None,
        })
        .fold(Ok(0), count_oks)? as u32;

    // SAFETY: TODO
    unsafe {
        set_nlink(inode, num_subdirs + EXFAT_MIN_SUBDIR);
    }

    inode.i_uid = sbi.info.options.fs_uid;
    inode.i_gid = sbi.info.options.fs_gid;

    // SAFETY: TODO
    unsafe {
        inode_inc_iversion(inode);
    }

    inode.i_generation = 0;
    inode.i_mode = FileAttributes::from_u16(ROOT_FILE_ATTRIBUTE).to_unix(0o777, sb_info);
    inode.i_op = &DIR_INODE_OPERATIONS;

    inode.__bindgen_anon_3.i_fop = unsafe { &DIR_OPERATIONS };

    // SAFETY: TODO
    let size = unsafe { i_size_read(inode) };
    inode.i_blocks =
        math::round_up_to_next_multiple(size as u64, sbi.info.boot_sector_info.cluster_size as u64)
            >> inode.i_blkbits;

    // SAFETY: TODO
    let curr_time = unsafe { current_time(inode) };
    inode.i_mtime = curr_time;
    inode.i_atime = curr_time;
    inode.i_ctime = curr_time;
    math::truncate_atime(&mut inode.i_atime);

    Ok(())
}
