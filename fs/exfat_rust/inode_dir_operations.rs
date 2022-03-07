use crate::charsets::{UTF16String, MAX_CHARSET_SIZE, MAX_NAME_LENGTH};
use crate::directory::{DirEntry, DirEntryReader};
use crate::inode::InodeExt;
use crate::superblock::take_sb;
use crate::superblock::SuperBlockInfo;
use kernel::bindings::{
    dentry as DEntry, inode as Inode, inode_operations as InodeOperations, umode_t,
    user_namespace as UserNamespace,
};
use kernel::c_types::{c_int, c_uint};
use kernel::prelude::*;
use kernel::Error;

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
fn find(sbi: &mut SuperBlockInfo<'_>, inode: Inode, name: String) -> Result<DirEntry> {
    if name.is_empty() {
        return Err(Error::ENOENT);
    }

    let utf16_name = resolve_path(sbi, name)?;

    todo!("Implement find_file");
}

// exfat_find_dir_entry in dir.c
fn find_dir(sbi: &mut SuperBlockInfo<'_>, inode: Inode, name: String) -> Result<DirEntry> {
    let inode = inode.to_info();
    let sb_info = &sbi.info;
    let sb_lock = sbi.state.as_ref().unwrap();
    let sb_state = sb_lock.lock();

    let reader = DirEntryReader::new(sb_info, &sb_state, inode.start_cluster)?;

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
fn resolve_path(sbi: &SuperBlockInfo<'_>, path: String) -> Result<UTF16String> {
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
    let utf16: UTF16String = UTF16String::from_nls(&sbi.info, &path, false)?;

    if utf16.0.len() == 0 {
        return Err(Error::EINVAL);
    }

    // TODO: Lossy handling

    return Ok(utf16);
}

extern "C" fn exfat_lookup(dir: *mut Inode, _dentry: *mut DEntry, _flags: c_uint) -> *mut DEntry {
    let inode = unsafe { &mut *dir };
    let inode = inode.to_info_mut();

    let sb = take_sb(&inode.vfs_inode.i_sb);

    let sb_state = sb.state.as_ref().unwrap().lock();

    // TODO: vvvvvvvvvvvvvvvvvvvv
    //	err = exfat_find(dir, &dentry->d_name, &info);
    //	if (err) {
    //		if (err == -ENOENT) {
    //			inode = NULL;
    //			goto out;
    //		}
    //		goto unlock;
    //	}
    //
    //	i_pos = exfat_make_i_pos(&info);
    //	inode = exfat_build_inode(sb, &info, i_pos);
    //	err = PTR_ERR_OR_ZERO(inode);
    //	if (err)
    //		goto unlock;
    //
    //	i_mode = inode->i_mode;
    //	alias = d_find_alias(inode);
    //
    //	/*
    //	 * Checking "alias->d_parent == dentry->d_parent" to make sure
    //	 * FS is not corrupted (especially double linked dir).
    //	 */
    //	if (alias && alias->d_parent == dentry->d_parent &&
    //			!exfat_d_anon_disconn(alias)) {
    //
    //		/*
    //		 * Unhashed alias is able to exist because of revalidate()
    //		 * called by lookup_fast. You can easily make this status
    //		 * by calling create and lookup concurrently
    //		 * In such case, we reuse an alias instead of new dentry
    //		 */
    //		if (d_unhashed(alias)) {
    //			WARN_ON(alias->d_name.hash_len !=
    //				dentry->d_name.hash_len);
    //			exfat_info(sb, "rehashed a dentry(%p) in read lookup",
    //				   alias);
    //			d_drop(dentry);
    //			d_rehash(alias);
    //		} else if (!S_ISDIR(i_mode)) {
    //			/*
    //			 * This inode has non anonymous-DCACHE_DISCONNECTED
    //			 * dentry. This means, the user did ->lookup() by an
    //			 * another name (longname vs 8.3 alias of it) in past.
    //			 *
    //			 * Switch to new one for reason of locality if possible.
    //			 */
    //			d_move(alias, dentry);
    //		}
    //		iput(inode);
    //		mutex_unlock(&EXFAT_SB(sb)->s_lock);
    //		return alias;
    //	}
    //	dput(alias);
    //out:
    //	mutex_unlock(&EXFAT_SB(sb)->s_lock);
    //	if (!inode)
    //		exfat_d_version_set(dentry, inode_query_iversion(dir));
    //
    //	return d_splice_alias(inode, dentry);
    //unlock:
    //	mutex_unlock(&EXFAT_SB(sb)->s_lock);
    //	return ERR_PTR(err);

    todo!("exfat_lookup: {:x}", unsafe { (*dir).i_ino }); // TODO: implement me
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
