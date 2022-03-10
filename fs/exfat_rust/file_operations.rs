use core::ptr::null_mut;
use kernel::bindings::{
    file, file_operations as FileOperations, generic_file_llseek, generic_file_mmap,
    generic_file_read_iter, generic_file_splice_read, generic_file_write_iter, iov_iter,
    iter_file_splice_write, kiocb, loff_t, pipe_inode_info, vm_area_struct,
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

extern "C" fn exfat_llseek(arg1: *mut file, arg2: loff_t, arg3: c_types::c_int) -> loff_t {
    todo!("TODO exfat_llseek");
}

extern "C" fn exfat_read_iter(arg1: *mut kiocb, arg2: *mut iov_iter) -> isize {
    todo!("TODO exfat_read_iter");
}

extern "C" fn exfat_write_iter(arg1: *mut kiocb, arg2: *mut iov_iter) -> isize {
    todo!("TODO exfat_write_iter");
}

extern "C" fn exfat_unlocked_ioctl(
    arg1: *mut file,
    arg2: c_types::c_uint,
    arg3: c_types::c_ulong,
) -> c_types::c_long {
    todo!("TODO exfat_unlocked_ioctl");
}

extern "C" fn exfat_compat_ioctl(
    arg1: *mut file,
    arg2: c_types::c_uint,
    arg3: c_types::c_ulong,
) -> c_types::c_long {
    todo!("TODO exfat_compat_ioctl");
}

extern "C" fn exfat_mmap(arg1: *mut file, arg2: *mut vm_area_struct) -> c_types::c_int {
    todo!("TODO exfat_mmap");
}

extern "C" fn exfat_fsync(
    arg1: *mut file,
    arg2: loff_t,
    arg3: loff_t,
    datasync: c_types::c_int,
) -> c_types::c_int {
    todo!("TODO exfat_fsync");
}

extern "C" fn exfat_splice_read(
    arg1: *mut file,
    arg2: *mut loff_t,
    arg3: *mut pipe_inode_info,
    arg4: usize,
    arg5: c_types::c_uint,
) -> isize {
    todo!("TODO exfat_splice_read");
}

extern "C" fn exfat_splice_write(
    arg1: *mut pipe_inode_info,
    arg2: *mut file,
    arg3: *mut loff_t,
    arg4: usize,
    arg5: c_types::c_uint,
) -> isize {
    todo!("TODO exfat_splice_write");
}
