use kernel::bindings::{sb_min_blocksize, super_block};
use crate::{get_exfat_sb_from_sb};
use crate::superblock::{BootSectorInfo, SuperBlockInfo};
use kernel::{Result, Error, pr_err};
use crate::external::sb_bread;

const JUMP_BOOT_VALUE: u32 = 0xEB7690;
const FILESYSTEM_NAME: &[u8] = b"EXFAT   ";
const BOOT_SIGNATURE: u16 = 0xAA55;

const MIN_BYTES_PER_SECTOR_SHIFT: u8 = 9;
const MAX_BYTES_PER_SECTOR_SHIFT: u8 = 9;

// Cluster 0 and 1 are reserved, the first cluster is 2 in the cluster heap.
const EXFAT_RESERVED_CLUSTERS: u32 = 2;

// 2^5=32 is the number of bytes per directory entry.
const DENTRY_SHIFT: u32 = 5;

const VOLUME_DIRTY_FLAG: u16 = 0x2;
const MEDIA_FAILURE_FLAG: u16 = 0x4;

const EXFAT_FIRST_CLUSTER: u32 = 2;
const EXFAT_CLUSTERS_UNTRACKED: u32 = !0;

#[repr(C)]
#[allow(dead_code)]
pub(crate) struct BootRegion {
    jump_boot: [u8; 3],
    filesystem_name: [u8; 8],
    must_be_zero: [u8; 53],
    partition_offset: u64,
    volume_length: u64,
    fat_offset: u32,
    fat_length: u32,
    cluster_heap_offset: u32,
    cluster_count: u32,
    first_cluster_of_root_directory: u32,
    volume_serial_number: u32,
    file_system_revision: u16,
    volume_flags: u16,
    bytes_per_sector_shift: u8,
    sectors_per_cluster_shift: u8,
    number_of_fats: u8,
    drive_select: u8,
    percent_in_use: u8,
    reserved: [u8; 7],
    boot_code: [u8; 390],
    boot_signature: u16,
}

pub(crate) fn read_boot_sector(sb: &mut super_block) -> Result<&mut SuperBlockInfo> {
    let sbi: &mut SuperBlockInfo = get_exfat_sb_from_sb!(sb);

    // TODO: We probably want to reimplement this function in Rust later on
    // Set block size to read super block
    // SAFETY: Lol errrrrh... It's C, what do you expect?
    unsafe { sb_min_blocksize(sb, 512) };
    let sector_size = sb.s_blocksize as usize;

    // The boot sector should be the first on the disk, read sector 0.
    let bh = sb_bread(sb, 0).ok_or_else(|| {
        pr_err!("unable to read boot sector");
        Error::EIO
    })?;

    let b_data = bh.b_data as *const BootRegion;
    let boot_region = unsafe { &*b_data };

    // TODO: Ensure conversion from little endian.
    if boot_region.boot_signature != BOOT_SIGNATURE {
        pr_err!("invalid boot record signature");
        return Err(Error::EINVAL);
    }

    if boot_region.filesystem_name != FILESYSTEM_NAME {
        pr_err!("invalid fs_name");
        return Err(Error::EINVAL);
    }

    // must_be_zero field must be filled with zero to prevent mounting from FAT volume.
    for b in boot_region.must_be_zero.iter() {
        if *b != 0 {
            pr_err!("must_be_zero is not zero");
            return Err(Error::EINVAL);
        }
    }

    if boot_region.number_of_fats != 1 && boot_region.number_of_fats != 2 {
        pr_err!("bogus number of FAT structures");
        return Err(Error::EINVAL);
    }

    if boot_region.bytes_per_sector_shift < MIN_BYTES_PER_SECTOR_SHIFT ||
       boot_region.bytes_per_sector_shift > MAX_BYTES_PER_SECTOR_SHIFT {
        pr_err!("bogus sector size bits {}", boot_region.bytes_per_sector_shift);
        return Err(Error::EINVAL);
    }

    if boot_region.sectors_per_cluster_shift > 25 - boot_region.bytes_per_sector_shift {
        pr_err!("bogus sectors per cluster : {}", boot_region.sectors_per_cluster_shift);
        return Err(Error::EINVAL);
    }

    let cluster_size_bits: u32 = (boot_region.sectors_per_cluster_shift + boot_region.bytes_per_sector_shift) as u32;
    let boot_sector_info = BootSectorInfo {
        num_sectors: boot_region.volume_length,
        num_clusters: boot_region.cluster_count + EXFAT_RESERVED_CLUSTERS,
        cluster_size: 1 << cluster_size_bits,
        cluster_size_bits,
        sect_per_clus: 1 << boot_region.sectors_per_cluster_shift,
        sect_per_clus_bits: boot_region.sectors_per_cluster_shift.into(),
        fat1_start_sector: boot_region.fat_offset.into(),
        fat2_start_sector: if boot_region.number_of_fats == 1 { None }
                           else { Some((boot_region.fat_offset + boot_region.fat_length).into()) },
        data_start_sector: boot_region.cluster_heap_offset.into(),
        num_fat_sectors: boot_region.fat_length,
        root_dir: boot_region.first_cluster_of_root_directory,
        dentries_per_clu: 1 << (cluster_size_bits - DENTRY_SHIFT),
        vol_flags: boot_region.volume_flags.into(),
        vol_flags_persistent: (boot_region.volume_flags & (VOLUME_DIRTY_FLAG | MEDIA_FAILURE_FLAG)).into(),
        clu_srch_ptr: EXFAT_FIRST_CLUSTER,
        used_clusters: EXFAT_CLUSTERS_UNTRACKED,
    };

    // Check consistencies
    if boot_sector_info.num_fat_sectors << boot_region.bytes_per_sector_shift <
        boot_sector_info.num_clusters * 4 {
        pr_err!("bogus fat length");
        return Err(Error::EINVAL);
    }

    if boot_sector_info.data_start_sector <
        boot_sector_info.fat1_start_sector +
            (boot_sector_info.num_fat_sectors * boot_region.number_of_fats as u32) as u64 {
        pr_err!("bogus data start sector");
        return Err(Error::EINVAL);
    }

    sbi.boot_sector_info = boot_sector_info;
    Ok(sbi)
}