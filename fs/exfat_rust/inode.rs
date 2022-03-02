use crate::directory::file::{FileAttributes, ROOT_FILE_ATTRIBUTE};
use crate::directory::{ExfatDirEntry, ExfatDirEntryReader};
use crate::fat::FatChainReader;
use crate::inode_dir_operations::DIR_INODE_OPERATIONS;
use crate::math;
use crate::superblock::SuperBlockInfo;
use alloc::vec::Vec;
use kernel::bindings::{current_time, i_size_read, i_size_write, inode_inc_iversion, set_nlink};
use kernel::Result;

pub(crate) type Inode = kernel::bindings::inode;

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

// C name `exfat_read_root`
pub(crate) fn read_root_inode(inode: &mut Inode, sbi: &mut SuperBlockInfo<'_>) -> Result {
    // TODO: We probably want to use this for something :shrug:
    let _info: &mut InodeInfo = inode.to_info_mut();

    let sb_info = &sbi.info;
    let sb_state = sbi.state.as_mut().unwrap().get_mut();
    let sb = &mut sb_state.sb;

    let root_dir = sb_info.boot_sector_info.root_dir;
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
    // TODO(Tux): inode->i_fop = &exfat_dir_operations;

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
