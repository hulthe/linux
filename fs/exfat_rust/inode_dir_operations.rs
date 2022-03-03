use kernel::bindings::{
    dentry as DEntry, inode as Inode, inode_operations as InodeOperations, umode_t,
    user_namespace as UserNamespace,
};
use kernel::c_types::{c_int, c_uint};

extern "C" fn exfat_create(
    _mnt_userns: *mut UserNamespace,
    _dir: *mut Inode,
    _dentry: *mut DEntry,
    _mode: umode_t,
    _excl: bool,
) -> c_int {
    todo!("exfat_create"); // TODO: implement me
}

extern "C" fn exfat_lookup(_dir: *mut Inode, _dentry: *mut DEntry, _flags: c_uint) -> *mut DEntry {
    todo!("exfat_lookup"); // TODO: implement me
}

extern "C" fn exfat_mkdir(
    _mnt_userns: *mut UserNamespace,
    _dir: *mut Inode,
    _dentry: *mut DEntry,
    _mode: umode_t,
) -> c_int {
    todo!("exfat_mkdir"); // TODO: implement me
}

extern "C" fn exfat_rmdir(_dir: *mut Inode, _dentry: *mut DEntry) -> c_int {
    todo!("exfat_rmdir"); // TODO: implement me
}

extern "C" fn exfat_unlink(_dir: *mut Inode, _dentry: *mut DEntry) -> c_int {
    todo!("exfat_unlink"); // TODO: implement me
}

extern "C" fn exfat_rename(
    _mnt_userns: *mut UserNamespace,
    _old_dir: *mut Inode,
    _old_dentry: *mut DEntry,
    _new_dir: *mut Inode,
    _new_dentry: *mut DEntry,
    _flags: c_uint,
) -> c_int {
    todo!("exfat_rename"); // TODO: implement me
}

pub(crate) static DIR_INODE_OPERATIONS: InodeOperations = InodeOperations {
    create: Some(exfat_create),
    lookup: Some(exfat_lookup),
    mkdir: Some(exfat_mkdir),
    rmdir: Some(exfat_rmdir),
    unlink: Some(exfat_unlink),
    rename: Some(exfat_rename),
    setattr: None, // TODO:
    getattr: None, // TODO:

    // Not implemented in C version, ignoring for now.
    get_link: None,
    permission: None,
    get_acl: None,
    readlink: None,
    link: None,
    symlink: None,
    mknod: None,
    listxattr: None,
    fiemap: None,
    update_time: None,
    atomic_open: None,
    tmpfile: None,
    set_acl: None,
    fileattr_set: None,
    fileattr_get: None,
};
