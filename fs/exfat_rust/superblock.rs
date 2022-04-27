use crate::allocation_bitmap::AllocationBitmap;
use crate::boot_sector::EXFAT_RESERVED_CLUSTERS;
use crate::fat::ClusterIndex;
use crate::inode::InodeHashTable;
use crate::upcase::UpcaseTable;
use kernel::bindings::{kgid_t, kuid_t, sector_t};
use kernel::c_types::{c_char, c_int, c_uint, c_ushort};
use kernel::prelude::*;
use kernel::sync::{Mutex, SpinLock};

pub(crate) type SuperBlock = kernel::bindings::super_block;

pub(crate) const NUM_RESERVED_CLUSTERS: u32 = 2;

pub(crate) fn take_sb<'a>(sb: &'a *mut SuperBlock) -> &'a SuperBlockInfo<'a> {
    unsafe { &*((**sb).s_fs_info as *mut SuperBlockInfo<'a>) }
}

// port of `exfat_sb_info` in exfat_fs.h
#[allow(dead_code)] // TODO
pub(crate) struct SuperBlockInfo<'a> {
    pub(crate) info: SbInfo,

    pub(crate) state: Mutex<SbState<'a>>,

    //struct buffer_head *boot_bh,
    /// allocation bitmap
    // TODO: Add lock
    pub(crate) allocation_bitmap: AllocationBitmap,

    // TODO: Inspect performance of this, original implementation used a hashtable of
    // Linked lists (for collisions?)
    pub(crate) inode_hashtable: SpinLock<InodeHashTable>,
    //struct rcu_head rcu,
}

pub(crate) struct SbInfo {
    /// The number of ExFatDirEntry that fits in a cluster
    pub(crate) dir_entries_per_cluster: u32,

    pub(crate) boot_sector_info: BootSectorInfo,

    pub(crate) options: ExfatMountOptions,

    /// UpCase table
    pub(crate) upcase_table: UpcaseTable,
    // /// Charset used for input and display
    //struct nls_table *nls_io,
    //struct ratelimit_state ratelimit,
}

pub(crate) struct SbState<'a> {
    /// SuperBlock
    pub(crate) sb: &'a mut SuperBlock,
}

pub(crate) trait SuperBlockExt {
    fn sectors_to_bytes(&self, sectors: u64) -> u64;
    fn bytes_to_sectors(&self, bytes: u64) -> u64;
}

impl SuperBlockExt for SuperBlock {
    fn sectors_to_bytes(&self, sectors: u64) -> u64 {
        sectors << self.s_blocksize_bits
    }

    fn bytes_to_sectors(&self, bytes: u64) -> u64 {
        ((bytes - 1) >> (self.s_blocksize_bits)) + 1
    }
}

impl SbInfo {
    pub(crate) fn cluster_to_sector(&self, cluster: ClusterIndex) -> sector_t {
        let sect_per_clus_bits = self.boot_sector_info.sect_per_clus_bits;
        let data_start_sector = self.boot_sector_info.data_start_sector;
        (((cluster - EXFAT_RESERVED_CLUSTERS) as sector_t) << sect_per_clus_bits)
            + data_start_sector
    }
}

#[allow(dead_code)] // TODO
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

impl BootSectorInfo {
    pub(crate) fn cluster_count(&self) -> u32 {
        self.num_clusters - NUM_RESERVED_CLUSTERS
    }
}

#[allow(dead_code)] // TODO
#[repr(C)]
pub(crate) enum ExfatErrorMode {
    Continue,
    Panic,
    RemountRo,
}

impl ExfatErrorMode {
    pub(crate) fn from_c_int(val: c_uint) -> Result<Self> {
        Ok(match val {
            0 => Self::Continue,
            1 => Self::Panic,
            2 => Self::RemountRo,
            _ => return Err(Error::EINVAL),
        })
    }

    pub(crate) const fn get_name(self) -> *const c_char {
        match self {
            ExfatErrorMode::Continue => b"continue\0".as_ptr() as *const i8,
            ExfatErrorMode::Panic => b"panic\0".as_ptr() as *const i8,
            ExfatErrorMode::RemountRo => b"remount-ro\0".as_ptr() as *const i8,
        }
    }
}

impl Default for ExfatErrorMode {
    fn default() -> Self {
        Self::Continue
    }
}

#[allow(dead_code)] // TODO
pub(crate) struct ExfatMountOptions {
    pub(crate) fs_uid: kuid_t,
    pub(crate) fs_gid: kgid_t,
    pub(crate) fs_fmask: c_ushort,
    pub(crate) fs_dmask: c_ushort,
    /* Permission for setting the [am]time*/
    pub(crate) allow_utime: c_ushort,
    pub(crate) iocharset: Box<[u8]>,
    pub(crate) errors: ExfatErrorMode,
    pub(crate) utf8: bool,
    pub(crate) discard: bool,
    pub(crate) time_offset: c_int,
}
