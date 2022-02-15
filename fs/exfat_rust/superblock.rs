// port of `exfat_sb_info` in exfat_fs.h
#[allow(dead_code)]
#[derive(Default)]
pub(crate) struct SuperBlockInfo {
    /// num of sectors in volume
    num_sectors: u64,
    /// num of clusters in volume
    num_clusters: u32,
    /// cluster size in bytes
    cluster_size: u32,
    cluster_size_bits: u32,
    /// cluster size in sectors
    sect_per_clus: u32,
    sect_per_clus_bits: u32,
    /// FAT1 start sector
    fat1_start_sector: u64,
    /// FAT2 start sector
    fat2_start_sector: u64,
    /// data area start sector
    data_start_sector: u64,
    /// num of FAT sectors
    num_fat_sectors: u32,
    /// root dir cluster
    root_dir: u32,
    /// num of dentries per cluster
    dentries_per_clu: u32,
    /// volume flags
    vol_flags: u32,
    /// volume flags to retain
    vol_flags_persistent: u32,
    // /// buffer_head of BOOT sector
    //struct buffer_head *boot_bh,
    /// allocation bitmap start cluster
    map_clu: u32,
    /// num of allocation bitmap sectors
    map_sectors: u32,
    /// allocation bitmap
    //struct buffer_head **vol_amap,

    // /// upcase table
    //unsigned short *vol_utbl,

    /// cluster search pointer
    clu_srch_ptr: u32,
    /// number of used clusters
    used_clusters: u32,
    // /// superblock lock
    //struct mutex s_lock,
    // /// bitmap lock
    //struct mutex bitmap_lock,
    //struct exfat_mount_options options,
    // /// Charset used for input and display
    //struct nls_table *nls_io,
    //struct ratelimit_state ratelimit,

    //spinlock_t inode_hash_lock,
    //struct hlist_head inode_hashtable[EXFAT_HASH_SIZE],

    //struct rcu_head rcu,
}
