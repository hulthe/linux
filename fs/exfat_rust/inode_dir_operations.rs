use kernel::bindings::inode_operations as InodeOperations;

pub(crate) static DIR_INODE_OPERATIONS: InodeOperations = InodeOperations {
    create: None,  // TODO:
    lookup: None,  // TODO:
    mkdir: None,   // TODO:
    rmdir: None,   // TODO:
    unlink: None,  // TODO:
    rename: None,  // TODO:
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
