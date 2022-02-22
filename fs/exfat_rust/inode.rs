use alloc::vec::Vec;
use kernel::{Result};
use crate::superblock::{SuperBlock, SuperBlockInfo};
use crate::fat::FatChainReader;

pub(crate) type Inode = kernel::bindings::inode;

const EXFAT_HASH_BITS: usize = 8;
const EXFAT_HASH_SIZE: usize = 1 << EXFAT_HASH_BITS;

#[allow(dead_code)]
pub(crate) struct InodeHashTable {
    inner: [Vec<Inode>; EXFAT_HASH_SIZE]
}

impl InodeHashTable {
    pub(crate) fn new() -> Self {
        const EMPTY: Vec<Inode> = Vec::new();
        Self {
            inner: [EMPTY; EXFAT_HASH_SIZE]
        }
    }
}

#[repr(C)]
pub(crate) struct InodeInfo {
    // SAFETY: vfs_inode MUST BE at the top of this struct,
    // otherwise hell will break lose and the angry angry
    // memory gods will forever be your nemesis.
    // DO NOT TOUCH!!!!!!!! (:cry:)!!!!!!!
    vfs_inode: Inode,
    // struct exfat_chain dir;
    entry: u32,
    // unsigned int type;
    // unsigned short attr;
    // unsigned int start_clu;
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

pub(crate) trait InodeExt {
    fn to_info(&self) -> &InodeInfo;
    fn to_info_mut(&mut self) -> &mut InodeInfo;
}

impl InodeExt for Inode {
    fn to_info(&self) -> &InodeInfo {
        let inode_info = self as *const _ as *const InodeInfo;
        unsafe { &*inode_info }
    }

    fn to_info_mut(&mut self) -> &mut InodeInfo {
        let inode_info = self as *mut _ as *mut InodeInfo;
        unsafe { &mut *inode_info }
    }
}

pub(crate) fn read_root_inode(inode: &mut Inode, super_block: &mut SuperBlock, exfat_sb_info: &mut SuperBlockInfo) -> Result {
    let info: &mut InodeInfo = inode.to_info_mut();

    let root_dir = exfat_sb_info.boot_sector_info.root_dir;
    let chain_reader = FatChainReader::new(super_block, root_dir);
    let num_clusters = chain_reader.fold(Ok(0), |bucket: Result<usize>, item| {
        let _ = item?;
        Ok(bucket? + 1)
    })?;

    // TODO: Finish function
    Ok(())
}