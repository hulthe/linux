use crate::null;
use kernel::bindings::{kgid_t, kuid_t};
use kernel::c_types;
use kernel::prelude::*;

// port of `exfat_sb_info` in exfat_fs.h
#[allow(dead_code)] // TODO
#[derive(Default)]
pub(crate) struct SuperBlockInfo {    // /// buffer_head of BOOT sector
    pub(crate) boot_sector_info: BootSectorInfo,
    //struct buffer_head *boot_bh,
    /// allocation bitmap start cluster
    pub(crate) map_clu: u32,
    /// num of allocation bitmap sectors
    pub(crate) map_sectors: u32,
    /// allocation bitmap
    //struct buffer_head **vol_amap,

    // /// upcase table
    pub(crate) vol_utbl: Option<Box<[u16]>>,

    // /// superblock lock
    //struct mutex s_lock,
    // /// bitmap lock
    //struct mutex bitmap_lock,
    pub(crate) options: ExfatMountOptions,
    // /// Charset used for input and display
    //struct nls_table *nls_io,
    //struct ratelimit_state ratelimit,

    //spinlock_t inode_hash_lock,
    //struct hlist_head inode_hashtable[EXFAT_HASH_SIZE],

    //struct rcu_head rcu,
}

#[derive(Default)]
pub(crate) struct BootSectorInfo {
    /// num of sectors in volume
    pub(crate) num_sectors: u64,
    /// num of clusters in volume
    pub(crate) num_clusters: u32,
    /// cluster size in bytes
    pub(crate) cluster_size: u32,
    pub(crate) cluster_size_bits: u32,
    /// cluster size in sectors
    pub(crate) sect_per_clus: u32,
    pub(crate) sect_per_clus_bits: u32,
    /// FAT1 start sector
    pub(crate) fat1_start_sector: u64,
    /// FAT2 start sector
    pub(crate) fat2_start_sector: Option<u64>,
    /// data area start sector
    pub(crate) data_start_sector: u64,
    /// num of FAT sectors
    pub(crate) num_fat_sectors: u32,
    /// root dir cluster
    pub(crate) root_dir: u32,
    /// num of dentries per cluster
    pub(crate) dentries_per_clu: u32,
    /// volume flags
    pub(crate) vol_flags: u32,
    /// volume flags to retain
    pub(crate) vol_flags_persistent: u32,
    /// cluster search pointer
    pub(crate) clu_srch_ptr: u32,
    /// number of used clusters
    pub(crate) used_clusters: u32,
}

#[allow(dead_code)] // TODO
#[repr(C)]
pub(crate) enum ExfatErrorMode {
    Continue,
    Panic,
    RemountRo,
}

impl ExfatErrorMode {
    pub(crate) const fn get_name(self) -> *const c_types::c_char {
        match self {
            ExfatErrorMode::Continue => b"continue\0".as_ptr() as *const i8,
            ExfatErrorMode::Panic => b"panic\0".as_ptr() as *const i8,
            ExfatErrorMode::RemountRo => b"remount-ro\0".as_ptr() as *const i8,
        }
    }
}

#[allow(dead_code)] // TODO
pub(crate) struct ExfatMountOptions {
    pub(crate) fs_uid: kuid_t,
    pub(crate) fs_gid: kgid_t,
    pub(crate) fs_fmask: c_types::c_ushort,
    pub(crate) fs_dmask: c_types::c_ushort,
    /* Permission for setting the [am]time*/
    pub(crate) allow_utime: c_types::c_ushort,
    pub(crate) iocharset: *const c_types::c_char,
    pub(crate) errors: ExfatErrorMode,
    pub(crate) utf8: bool,
    pub(crate) discard: bool,
    pub(crate) time_offset: c_types::c_int,
}

impl Default for ExfatMountOptions {
    fn default() -> Self {
        Self {
            fs_uid: kuid_t::default(),
            fs_gid: kgid_t::default(),
            fs_fmask: 0,
            fs_dmask: 0,
            allow_utime: 0,
            iocharset: null(),
            errors: ExfatErrorMode::Continue,
            utf8: true,
            discard: true,
            time_offset: 0,
        }
    }
}
