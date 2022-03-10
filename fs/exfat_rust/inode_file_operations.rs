use kernel::bindings::{
    address_space, address_space_operations as AddressSpaceOperations, dentry, file, iattr,
    inode_operations as InodeOperations, iov_iter, kiocb, kstat, list_head, loff_t, page, path,
    readahead_control, sector_t, u32_, user_namespace, writeback_control,
};
use kernel::c_types;

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
    direct_IO: Some(exfat_direct_IO),
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

extern "C" fn exfat_direct_IO(arg1: *mut kiocb, iter: *mut iov_iter) -> isize {
    todo!("TODO exfat_readahead");
}
