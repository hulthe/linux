pub(crate) mod hash_table;

pub(crate) use self::hash_table::InodeHashTable;

use crate::directory::file::{FileAttributes, ROOT_FILE_ATTRIBUTE};
use crate::directory::{DirEntry, ExFatDirEntryKind, ExFatDirEntryReader};
use crate::fat::FatChainReader;
use crate::file_operations::FILE_OPERATIONS;
use crate::file_ops::DIR_OPERATIONS;
use crate::inode_dir_operations::DIR_INODE_OPERATIONS;
use crate::inode_file_operations::{ADDRESS_OPERATIONS, FILE_INODE_OPERATIONS};
use crate::kmem_cache::KMemCache;
use crate::kmem_cache::PtrInit;
use crate::math::{self, round_up_to_next_multiple};
use crate::superblock::{SbInfo, SbState, SuperBlock, SuperBlockInfo};
use crate::EXFAT_ROOT_INO;
use core::mem::align_of;
use core::ptr::{null_mut, NonNull};
use kernel::bindings::{
    self, __insert_inode_hash, current_time, i_size_read, i_size_write, inode_inc_iversion,
    inode_init_once, inode_set_iversion, iunique, loff_t, new_inode, prandom_u32, set_nlink,
    ___GFP_DIRECT_RECLAIM, ___GFP_IO, ___GFP_KSWAPD_RECLAIM,
};
use kernel::linked_list::Links;
use kernel::sync::SpinLock;
use kernel::{Error, Result};

pub(crate) type Inode = kernel::bindings::inode;

// TODO: consider making this not a global. e.g. by putting it in the superblock
/// Cache allocations of InodeInfo:s
pub(crate) static INODE_ALLOC_CACHE: KMemCache<InodeInfo> = KMemCache::new();

pub(crate) fn inode_unique_num(cluster: u32, entry: u32) -> u64 {
    (entry as u64) << u32::BITS | cluster as u64
}

#[repr(C)]
pub(crate) struct InodeInfo {
    // SAFETY: vfs_inode MUST BE at the top of this struct,
    // otherwise hell will break lose and the angry angry
    // memory gods will forever be your nemesis.
    // DO NOT TOUCH!!!!!!!! (:cry:)!!!!!!!
    pub(crate) vfs_inode: Inode,
    // struct exfat_chain dir;
    /// The start of the cluster chain that contains the directory entry for this inode
    pub(crate) dir_cluster: u32,

    /// The ExFatDirEntry index in the cluster chain
    pub(crate) entry_index: u32,

    /// The start of the cluster chain that contains the data for this inode
    pub(crate) data_cluster: u32,

    pub(crate) size_aligned: u64,
    pub(crate) size_ondisk: u64,

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
    inode_cache_list: Links<Self>,
}

impl InodeInfo {
    /// Get the unique number that identifies this Inode
    pub(crate) fn unique_num(&self) -> u64 {
        inode_unique_num(self.dir_cluster, self.entry_index)
    }

    fn fill(&mut self, sb_info: &SbInfo, sb_state: &SbState<'_>, dir: &DirEntry) {
        self.dir_cluster = dir.cluster;
        self.entry_index = dir.index;
        self.data_cluster = dir.data_cluster;
        //ei->dir = info->dir;
        //ei->entry = info->entry;
        //ei->attr = info->attr;
        //ei->start_clu = info->start_clu;
        //ei->flags = info->flags;
        //ei->type = info->type;

        //ei->version = 0;
        //ei->hint_stat.eidx = 0;
        //ei->hint_stat.clu = info->start_clu;
        //ei->hint_femp.eidx = EXFAT_HINT_NONE;
        //ei->hint_bmap.off = EXFAT_EOF_CLUSTER;
        //ei->i_pos = 0;

        self.vfs_inode.i_uid = sb_info.options.fs_uid;
        self.vfs_inode.i_gid = sb_info.options.fs_gid;
        unsafe { inode_inc_iversion(&mut self.vfs_inode) };
        self.vfs_inode.i_generation = unsafe { prandom_u32() };

        if dir.attrs.directory() {
            self.vfs_inode.i_generation &= !1u32; // unset the lowest bit
            self.vfs_inode.i_mode = dir.attrs.to_unix(0o777, sb_info);
            self.vfs_inode.i_op = &DIR_INODE_OPERATIONS;
            self.vfs_inode.__bindgen_anon_3.i_fop = unsafe { &DIR_OPERATIONS };

            // set_nlink(&mut self.vfs_inode, dir.num_subdirs); // TODO
            unsafe { set_nlink(&mut self.vfs_inode, 0) };
        } else {
            // regular file
            self.vfs_inode.i_generation |= 1; // set the lowest bit
            self.vfs_inode.i_mode = dir.attrs.to_unix(0o777, sb_info);
            self.vfs_inode.i_op = &FILE_INODE_OPERATIONS as *const _;
            // SAFETY: TODO
            self.vfs_inode.__bindgen_anon_3.i_fop = unsafe { &FILE_OPERATIONS as *const _ };

            let i_mapping = unsafe { &mut *self.vfs_inode.i_mapping };
            // SAFETY: TODO
            i_mapping.a_ops = &ADDRESS_OPERATIONS as *const _;
            i_mapping.nrpages = 0;
        }

        // TODO: make sure data_length is what we're supposed to be using
        let mut size = dir.data_length;
        unsafe { i_size_write(&mut self.vfs_inode, size as i64) };

        // ondisk and aligned size should be aligned with block size
        if size & (sb_state.sb.s_blocksize - 1) != 0 {
            size |= sb_state.sb.s_blocksize - 1;
            size += 1;
        }

        self.size_aligned = size;
        self.size_ondisk = size;

        // exfat_save_attr(inode, dir.attrs) // TODO

        self.vfs_inode.i_blocks = round_up_to_next_multiple(
            unsafe { i_size_read(&self.vfs_inode) } as u64,
            sb_info.boot_sector_info.cluster_size as u64,
        ) >> self.vfs_inode.i_blkbits;

        self.vfs_inode.i_mtime = dir.modified_time;
        self.vfs_inode.i_ctime = dir.modified_time; // TODO: unsure why ctime is set to mtime here?
        self.vfs_inode.i_atime = dir.access_time;
        //self.i_crtime = dir.create_time; // TODO
    }

    /// Get an inode from the cache, or create a new one of it doesn't exist.
    pub(crate) fn build<'a>(
        sb_state: &mut SbState<'_>,
        sb_info: &SbInfo,
        inode_hashtable: &SpinLock<InodeHashTable>,
        dir: &DirEntry,
    ) -> Result<&'a mut Self> {
        if let Some(inode) = inode_hashtable.lock().get(dir.cluster, dir.index) {
            return Ok(inode);
        }

        // SAFETY: TODO
        let inode = unsafe { new_inode(sb_state.sb).as_mut() }.ok_or(Error::ENOMEM)?;

        inode.i_ino = unsafe { iunique(sb_state.sb, EXFAT_ROOT_INO) };
        unsafe { inode_set_iversion(inode, 1) };

        let inode = inode.to_info_mut();
        inode.fill(sb_info, sb_state, dir);

        inode_hashtable.lock().insert(inode);
        unsafe { __insert_inode_hash(&mut inode.vfs_inode, inode.unique_num()) };

        Ok(inode)
    }
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
            dir_cluster: 0,
            entry_index: 0,

            data_cluster: 0,

            size_aligned: 0,
            size_ondisk: 0,

            inode_cache_list: Links::new(),
        };

        unsafe { ptr.as_ptr().write(inode) };
    }
}

pub(crate) trait InodeExt {
    fn to_info(&self) -> &InodeInfo;
    fn to_info_mut(&mut self) -> &mut InodeInfo;
    fn i_size_read(&self) -> loff_t;
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

    fn i_size_read(&self) -> loff_t {
        unsafe { bindings::i_size_read(self) }
    }
}

// Representing `.` and `..`?
const EXFAT_MIN_SUBDIR: u32 = 2;

pub(crate) extern "C" fn alloc_inode(_sb: *mut SuperBlock) -> *mut Inode {
    kernel::pr_info!("alloc_inode called");
    // bindgen is confused by these constants. // TODO move them
    const __GFP_RECLAIM: u32 = ___GFP_DIRECT_RECLAIM | ___GFP_KSWAPD_RECLAIM;
    const GFP_NOFS: u32 = __GFP_RECLAIM | ___GFP_IO;
    if let Ok(ei) = INODE_ALLOC_CACHE.alloc(GFP_NOFS) {
        // TODO: initialize locks
        unsafe { &mut (*ei.as_ptr()).vfs_inode }
    } else {
        null_mut()
    }
}
pub(crate) extern "C" fn free_inode(inode: *mut Inode) {
    kernel::pr_info!("free_inode called");
    if let Some(inode) = NonNull::new(inode as *mut InodeInfo) {
        unsafe { INODE_ALLOC_CACHE.free(inode) };
    }
}

// C name `exfat_read_root`
pub(crate) fn read_root_inode(inode: &mut Inode, sbi: &mut SuperBlockInfo<'_>) -> Result {
    // TODO: We probably want to use this for something :shrug:
    let info: &mut InodeInfo = inode.to_info_mut();
    let inode = &mut info.vfs_inode;

    let sb_info = &sbi.info;
    let sb_state = sbi.state.get_mut();
    let sb = &mut sb_state.sb;

    let root_dir = sb_info.boot_sector_info.root_dir;
    info.dir_cluster = 0; // TODO
    info.data_cluster = root_dir;
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

    let dir_reader = ExFatDirEntryReader::new(&sb_info.boot_sector_info, sb_state, root_dir)?;
    let num_subdirs = dir_reader
        .filter_map(|dir_entry| match dir_entry.map(|e| e.kind) {
            Err(e) => Some(Err(e)),
            Ok(ExFatDirEntryKind::File(file)) => file.file_attributes.directory().then(|| Ok(())),
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
        round_up_to_next_multiple(size as u64, sbi.info.boot_sector_info.cluster_size as u64)
            >> inode.i_blkbits;

    // SAFETY: TODO
    let curr_time = unsafe { current_time(inode) };
    inode.i_mtime = curr_time;
    inode.i_atime = curr_time;
    inode.i_ctime = curr_time;
    math::truncate_atime(&mut inode.i_atime);

    Ok(())
}
