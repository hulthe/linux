use crate::charsets::{MAX_CHARSET_SIZE, MAX_NAME_LENGTH};
use crate::directory::{DirEntry, DirEntryReader};
use crate::inode::{InodeExt, InodeInfo};
use crate::superblock::{take_sb, SbInfo, SbState, SuperBlockInfo};
use core::ptr::null_mut;
use kernel::bindings::{
    d_drop, d_find_alias, d_move, d_rehash, d_splice_alias, d_unhashed, dentry as DEntry, dput,
    inode as Inode, inode_operations as InodeOperations, iput, umode_t,
    user_namespace as UserNamespace, DCACHE_DISCONNECTED,
};
use kernel::c_types::{c_int, c_uint};
use kernel::prelude::*;
use kernel::{from_kernel_err_ptr, Error};

extern "C" fn exfat_create(
    _mnt_userns: *mut UserNamespace,
    _dir: *mut Inode,
    _dentry: *mut DEntry,
    _mode: umode_t,
    _excl: bool,
) -> c_int {
    todo!("exfat_create"); // TODO: implement me
}

// exfat_find in namei.c
fn find<'a>(
    sb_info: &'a SbInfo,
    sb_state: &'a SbState<'a>,
    inode: &InodeInfo,
    name: String,
) -> Result<DirEntry> {
    if name.is_empty() {
        return Err(Error::ENOENT);
    }

    let name = resolve_path(sb_info, sb_state, name)?;
    return find_dir(sb_info, sb_state, inode, &name);
}

// exfat_find_dir_entry in dir.c
fn find_dir<'a>(
    sb_info: &'a SbInfo,
    sb_state: &'a SbState<'a>,
    inode: &InodeInfo,
    name: &str,
) -> Result<DirEntry> {
    let reader = DirEntryReader::new(sb_info, sb_state, inode.data_cluster)?;

    // TODO: Add name hashing optimization & hint optimization.

    for entry in reader {
        let entry = entry?;

        if entry.name == name {
            return Ok(entry);
        }
    }

    Err(Error::ENOENT)
}

/// Lookup a path in the given inode, if it exists return Ok with the UTF16 version of the name.
// exfat_resolve_path_for_lookup in namei.c
fn resolve_path<'a>(
    _sb_info: &'a SbInfo,
    sb_state: &'a SbState<'a>,
    path: String,
) -> Result<String> {
    // Remove trailing periods.
    let stripped = path.trim_end_matches(".");
    if stripped.is_empty() {
        return Err(Error::ENOENT);
    }

    if path.len() > (MAX_NAME_LENGTH as usize * MAX_CHARSET_SIZE as usize) {
        return Err(Error::ENAMETOOLONG);
    }

    // Here we should strip all leading spaces :
    // "MS windows 7" supports leading spaces,
    // so we should skip these for compatability reasons.

    // File name conversion
    // let utf16: UTF16String = UTF16String::from_nls(&sbi.info, &path, false)?;

    // I guess this is not needed?
    // if utf16.0.len() == 0 {
    //     return Err(Error::EINVAL);
    // }

    // TODO: Lossy handling

    return Ok(path);
    // return Ok(utf16);
}

fn lookup<'a>(
    dir_inode: &mut InodeInfo,
    sbi: &'a SuperBlockInfo<'a>,
    dentry: &mut DEntry,
    path_name: &CStr,
) -> Result<Option<&'static mut DEntry>> {
    let mut sb_state = sbi.state.as_ref().unwrap().lock();
    let sb_info = &sbi.info;

    let path_name = path_name.to_str()?.try_to_owned()?;
    let dir_entry = find(sb_info, &sb_state, dir_inode, path_name)?;
    let is_dir = dir_entry.attrs.directory();

    let inode = InodeInfo::build(&mut sb_state, sb_info, &sbi.inode_hashtable, &dir_entry)?;
    // TODO: call exfat_d_version_set if inode failed with ENOENT

    pr_info!("dir_inode i_ino: {:?}", dir_inode.vfs_inode.i_ino);
    pr_info!("dir_inode data_cluster: {:?}", dir_inode.data_cluster);
    pr_info!("dir_inode dir_cluster: {:?}", dir_inode.dir_cluster);
    pr_info!("looked up {}", dir_entry.name);
    pr_info!("dir_entry data_cluster {}", dir_entry.data_cluster);
    pr_info!("inode data_cluster {}", inode.data_cluster);
    pr_info!("inode dir_cluster {}", inode.dir_cluster);

    //pr_info!("dir i_mode: {:?}", dir_inode.vfs_inode.i_mode);
    //pr_info!("dir i_opflags: {:?}", dir_inode.vfs_inode.i_opflags);
    //pr_info!("dir i_flags: {:?}", dir_inode.vfs_inode.i_flags);
    //pr_info!("dir i_acl: {:?}", dir_inode.vfs_inode.i_acl);
    //pr_info!("dir i_default_acl: {:?}", dir_inode.vfs_inode.i_default_acl);
    //pr_info!("dir i_op: {:?}", dir_inode.vfs_inode.i_op);
    //pr_info!("dir i_sb: {:?}", dir_inode.vfs_inode.i_sb);
    //pr_info!("dir i_mapping: {:?}", dir_inode.vfs_inode.i_mapping);
    //pr_info!("dir i_security: {:?}", dir_inode.vfs_inode.i_security);
    //pr_info!("dir i_rdev: {:?}", dir_inode.vfs_inode.i_rdev);
    //pr_info!("dir i_size: {:?}", dir_inode.vfs_inode.i_size);
    //pr_info!("dir i_bytes: {:?}", dir_inode.vfs_inode.i_bytes);
    //pr_info!("dir i_blkbits: {:?}", dir_inode.vfs_inode.i_blkbits);
    //pr_info!("dir i_write_hint: {:?}", dir_inode.vfs_inode.i_write_hint);
    //pr_info!("dir i_blocks: {:?}", dir_inode.vfs_inode.i_blocks);
    //pr_info!("dir i_state: {:?}", dir_inode.vfs_inode.i_state);
    //pr_info!("dir dirtied_when: {:?}", dir_inode.vfs_inode.dirtied_when);
    //pr_info!(
    //    "dir dirtied_time_when: {:?}",
    //    dir_inode.vfs_inode.dirtied_time_when
    //);
    //pr_info!("dir i_flctx: {:?}", dir_inode.vfs_inode.i_flctx);
    //pr_info!("dir i_generation: {:?}", dir_inode.vfs_inode.i_generation);
    //pr_info!(
    //    "dir i_fsnotify_mask: {:?}",
    //    dir_inode.vfs_inode.i_fsnotify_mask
    //);
    //pr_info!(
    //    "dir i_fsnotify_marks: {:?}",
    //    dir_inode.vfs_inode.i_fsnotify_marks
    //);
    //pr_info!("dir i_private: {:?}", dir_inode.vfs_inode.i_private);

    let alias = unsafe { d_find_alias(&mut inode.vfs_inode) };
    if let Some(alias) = unsafe { alias.as_mut() } {
        // Checking "alias->d_parent == dentry->d_parent" to make sure
        // FS is not corrupted (especially double linked dir).
        if alias.d_parent == dentry.d_parent && !d_anon_disconn(alias) {
            // Unhashed alias is able to exist because of revalidate()
            // called by lookup_fast. You can easily make this status
            // by calling create and lookup concurrently
            // In such case, we reuse an alias instead of new dentry
            if unsafe { d_unhashed(alias) } {
                // WARN_ON(alias->d_name.hash_len != dentry->d_name.hash_len);
                // exfat_info(sb, "rehashed a dentry(%p) in read lookup", alias);
                unsafe { d_drop(dentry) };
                unsafe { d_rehash(alias) };
            } else if !is_dir {
                // This inode has non anonymous-DCACHE_DISCONNECTED
                // dentry. This means, the user did ->lookup() by an
                // another name (longname vs 8.3 alias of it) in past.
                //
                // Switch to new one for reason of locality if possible.
                unsafe { d_move(alias, dentry) };
            }
            unsafe { iput(&mut inode.vfs_inode) };
            return Ok(Some(alias));
        }
    }

    unsafe { dput(alias) };

    // TODO: figure out this stuff vvvv
    //out:
    //    mutex_unlock(&EXFAT_SB(sb)->s_lock);
    //    if (!inode)
    //        exfat_d_version_set(dentry, inode_query_iversion(dir));
    //
    //    return d_splice_alias(inode, dentry);

    let _ = sb_state; // drop superblock lock
    unsafe { Ok(from_kernel_err_ptr(d_splice_alias(&mut inode.vfs_inode, dentry))?.as_mut()) }
}

#[inline]
fn d_is_root(dentry: &DEntry) -> bool {
    dentry as *const _ == dentry.d_parent
}

#[inline]
fn d_anon_disconn(dentry: &DEntry) -> bool {
    d_is_root(dentry) && (dentry.d_flags & DCACHE_DISCONNECTED) != 0
}

extern "C" fn exfat_lookup(inode: *mut Inode, dentry: *mut DEntry, _flags: c_uint) -> *mut DEntry {
    // SAFETY: TODO
    let inode = unsafe { &mut *inode };
    // SAFETY: No idea. TODO
    let path_name = unsafe { &CStr::from_char_ptr((*dentry).d_name.name as *const i8) };
    let inode = inode.to_info_mut();
    let dentry = unsafe { &mut *dentry };
    let sb = inode.vfs_inode.i_sb;
    let sbi = take_sb(&sb);

    match lookup(inode, sbi, dentry, path_name) {
        Ok(Some(dentry)) => dentry,
        Ok(None) => null_mut(),
        Err(err) => {
            pr_err!(
                "ERROR ERROR FUCK I DONT KNOW WHAT TO DO WITH IT AWHMYGAWD. {:?}",
                err
            );
            err.to_kernel_errno() as *mut _
        }
    }
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
