use core::ptr::null_mut;
use kernel::bindings::{
    file, file_operations as FileOperations, generic_file_llseek, generic_file_mmap,
    generic_file_read_iter, generic_file_splice_read, generic_file_write_iter,
    iter_file_splice_write, loff_t,
};
use kernel::c_types;

pub(crate) static mut FILE_OPERATIONS: FileOperations = FileOperations {
    llseek: Some(generic_file_llseek),
    read_iter: Some(generic_file_read_iter),
    unlocked_ioctl: Some(exfat_unlocked_ioctl),

    #[cfg(CONFIG_COMPAT)]
    compat_ioctl: Some(exfat_compat_ioctl),

    mmap: Some(generic_file_mmap),
    fsync: Some(exfat_fsync),
    splice_read: Some(generic_file_splice_read),

    // Probably won't need for read-only
    write_iter: Some(generic_file_write_iter),
    splice_write: Some(iter_file_splice_write),

    // Not included in C version.
    owner: null_mut(),
    check_flags: None,
    read: None,
    write: None,
    iopoll: None,
    iterate: None,
    iterate_shared: None,
    poll: None,
    mmap_supported_flags: 0,
    open: None,
    flush: None,
    release: None,
    fasync: None,
    lock: None,
    sendpage: None,
    get_unmapped_area: None,
    flock: None,
    setlease: None,
    fallocate: None,
    show_fdinfo: None,
    copy_file_range: None,
    remap_file_range: None,
    fadvise: None,
};

extern "C" fn exfat_unlocked_ioctl(
    _arg1: *mut file,
    _arg2: c_types::c_uint,
    _arg3: c_types::c_ulong,
) -> c_types::c_long {
    todo!("TODO exfat_unlocked_ioctl");
}

extern "C" fn exfat_compat_ioctl(
    _arg1: *mut file,
    _arg2: c_types::c_uint,
    _arg3: c_types::c_ulong,
) -> c_types::c_long {
    todo!("TODO exfat_compat_ioctl");
}

extern "C" fn exfat_fsync(
    _arg1: *mut file,
    _arg2: loff_t,
    _arg3: loff_t,
    _datasync: c_types::c_int,
) -> c_types::c_int {
    todo!("TODO exfat_fsync");
}
