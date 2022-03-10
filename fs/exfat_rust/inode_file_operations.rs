use crate::from_kernel_result;
use crate::inode::{Inode, InodeExt, InodeInfo};
use crate::superblock::{take_sb, SuperBlockExt, SuperBlockInfo};
use core::cmp::min;
use kernel::bindings::{
    address_space, address_space_operations as AddressSpaceOperations, buffer_head as BufferHead,
    dentry, file, iattr, inode_operations as InodeOperations, iov_iter, kiocb, kstat, list_head,
    loff_t, map_bh, page, path, readahead_control, sector_t, u32_, user_namespace,
    writeback_control,
};
use kernel::c_types;
use kernel::c_types::c_int;
use kernel::Result;

pub(crate) static FILE_INODE_OPERATIONS: InodeOperations = InodeOperations {
    getattr: None, // Doesn't appear like we need this, implement if necessary

    // Probably won't need for read-only?
    setattr: Some(exfat_setattr),

    // Not implemented in C either
    lookup: None,
    get_link: None,
    permission: None,
    get_acl: None,
    readlink: None,
    create: None,
    link: None,
    unlink: None,
    symlink: None,
    mkdir: None,
    rmdir: None,
    mknod: None,
    rename: None,
    listxattr: None,
    fiemap: None,
    atomic_open: None,
    tmpfile: None,
    set_acl: None,
    fileattr_set: None,
    fileattr_get: None,
    update_time: None,
};

extern "C" fn exfat_getattr(
    arg1: *mut user_namespace,
    arg2: *const path,
    arg3: *mut kstat,
    arg4: u32_,
    arg5: c_types::c_uint,
) -> c_types::c_int {
    todo!("TODO exfat_getattr");
}

extern "C" fn exfat_setattr(
    arg1: *mut user_namespace,
    arg2: *mut dentry,
    arg3: *mut iattr,
) -> c_types::c_int {
    todo!("TODO exfat_setattr");
}

pub(crate) static ADDRESS_OPERATIONS: AddressSpaceOperations = AddressSpaceOperations {
    readpage: Some(exfat_readpage),
    readpages: Some(exfat_readpages),
    readahead: Some(exfat_readahead),

    bmap: Some(exfat_bmap),

    // Probably out of scope
    writepage: Some(exfat_writepage),
    writepages: Some(exfat_writepages),
    write_begin: Some(exfat_write_begin),
    write_end: Some(exfat_write_end),
    direct_IO: Some(exfat_direct_io),
    set_page_dirty: Some(exfat_set_page_dirty),

    // Not implemented in exfat either, ignore
    invalidatepage: None,
    releasepage: None,
    freepage: None,
    migratepage: None,
    isolate_page: None,
    putback_page: None,
    launder_page: None,
    is_partially_uptodate: None,
    is_dirty_writeback: None,
    error_remove_page: None,
    swap_activate: None,
    swap_deactivate: None,
};

extern "C" fn exfat_writepage(page: *mut page, wbc: *mut writeback_control) -> c_types::c_int {
    todo!("TODO exfat_writepage");
}

extern "C" fn exfat_readpage(arg1: *mut file, arg2: *mut page) -> c_types::c_int {
    todo!("TODO exfat_readpage");
}

extern "C" fn exfat_writepages(
    arg1: *mut address_space,
    arg2: *mut writeback_control,
) -> c_types::c_int {
    todo!("TODO exfat_writepages");
}

extern "C" fn exfat_set_page_dirty(page: *mut page) -> c_types::c_int {
    todo!("TODO exfat_set_page_dirty");
}

extern "C" fn exfat_readpages(
    filp: *mut file,
    mapping: *mut address_space,
    pages: *mut list_head,
    nr_pages: c_types::c_uint,
) -> c_types::c_int {
    todo!("TODO exfat_readpages");
}

extern "C" fn exfat_readahead(arg1: *mut readahead_control) {
    todo!("TODO exfat_readahead");
}

extern "C" fn exfat_write_begin(
    arg1: *mut file,
    mapping: *mut address_space,
    pos: loff_t,
    len: c_types::c_uint,
    flags: c_types::c_uint,
    pagep: *mut *mut page,
    fsdata: *mut *mut c_types::c_void,
) -> c_types::c_int {
    todo!("TODO exfat_write_begin");
}

extern "C" fn exfat_write_end(
    arg1: *mut file,
    mapping: *mut address_space,
    pos: loff_t,
    len: c_types::c_uint,
    copied: c_types::c_uint,
    page: *mut page,
    fsdata: *mut c_types::c_void,
) -> c_types::c_int {
    todo!("TODO exfat_readahead");
}

extern "C" fn exfat_bmap(arg1: *mut address_space, arg2: sector_t) -> sector_t {
    todo!("TODO exfat_readahead");
}

extern "C" fn exfat_direct_io(arg1: *mut kiocb, iter: *mut iov_iter) -> isize {
    todo!("TODO exfat_readahead");
}

extern "C" fn exfat_get_block(
    inode: *mut Inode,
    iblock: sector_t,
    bh_result: *mut BufferHead,
    create: bool,
) -> c_int {
    let inode = unsafe { &mut *inode };
    let inode = inode.to_info_mut();
    let sb = inode.vfs_inode.i_sb;
    let sbi = take_sb(&sb);
    let bh_result = unsafe { &mut *bh_result };
    from_kernel_result! { get_block(inode, sbi, iblock, bh_result, create) }
}

fn get_block(
    inode: &mut InodeInfo,
    sbi: &SuperBlockInfo<'_>,
    iblock: sector_t,
    bh_result: &mut BufferHead,
    create: bool,
) -> Result {
    if create {
        unimplemented!("get_block(create=true)")
    }

    let mut sb_state = sbi.state.as_ref().unwrap().lock();
    let sb = &mut *sb_state.sb;
    let sb_info = &sbi.info;

    let max_blocks = (bh_result.b_size >> inode.vfs_inode.i_blkbits) as u64;
    let i_size = inode.vfs_inode.i_size_read() as u64;
    let last_block = sb.bytes_to_sectors(i_size);

    if iblock >= last_block
    /* && !create */
    {
        bh_result.b_size = sb_state.sb.sectors_to_bytes(max_blocks) as usize;
        return Ok(());
    }

    // TODO: tmp code
    let cluster = (iblock >> sb_info.boot_sector_info.sect_per_clus_bits) as u32;
    // /* Is this block already allocated? */
    //	err = exfat_map_cluster(inode, iblock >> sbi->sect_per_clus_bits,
    //			&cluster, create);
    //	if (err) {
    //		if (err != -ENOSPC)
    //			exfat_fs_error_ratelimit(sb,
    //				"failed to bmap (inode : %p iblock : %llu, err : %d)",
    //				inode, (unsigned long long)iblock, err);
    //		goto unlock_ret;
    //	}
    //
    //	if (cluster == EXFAT_EOF_CLUSTER)
    //		goto done;

    // sector offset in cluster
    let sec_offset = iblock & (sb_info.boot_sector_info.sect_per_clus - 1) as u64;

    let phys = sb_info.cluster_to_sector(cluster) + sec_offset;
    let mapped_blocks = sb_info.boot_sector_info.sect_per_clus as u64 - sec_offset;
    let max_blocks = min(mapped_blocks, max_blocks);

    //	/* Treat newly added block / cluster */
    //	if (iblock < last_block)
    //		create = 0;

    //	if (create || buffer_delay(bh_result)) {
    //		pos = EXFAT_BLK_TO_B((iblock + 1), sb);
    //		if (ei->i_size_ondisk < pos)
    //			ei->i_size_ondisk = pos;
    //	}

    //	if (create) {
    //		err = exfat_map_new_buffer(ei, bh_result, pos);
    //		if (err) {
    //			exfat_fs_error(sb,
    //					"requested for bmap out of range(pos : (%llu) > i_size_aligned(%llu)\n",
    //					pos, ei->i_size_aligned);
    //			goto unlock_ret;
    //		}
    //	}

    //if buffer_delay(bh_result) {
    //    clear_buffer_delay(bh_result);
    //}

    unsafe { map_bh(bh_result, sb, phys) };

    Ok(())
}
