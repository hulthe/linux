use crate::boot_sector::EXFAT_CLUSTERS_UNTRACKED;
use crate::from_kernel_result;
use crate::inode::{alloc_inode, free_inode, InodeExt};
use crate::superblock::{take_sb, ExfatErrorMode};
use crate::zeroed;
use kernel::bindings::{
    clear_inode, dentry as Dentry, huge_encode_dev, i_size_write, inode as Inode,
    invalidate_inode_buffers, kstatfs as KStatFs, seq_file as SeqFile, seq_printf,
    super_block as SuperBlock, super_operations as SuperOperations, sync_blockdev,
    truncate_inode_pages, u64_to_fsid, writeback_control as WritebackControl,
};
use kernel::c_types::{c_int, c_uint};

pub(crate) static mut EXFAT_SOPS: SuperOperations = SuperOperations {
    alloc_inode: Some(alloc_inode),
    free_inode: Some(free_inode),
    //destroy_inode: Some(exfat_destroy_inode), // (Not in C version but we might need it to ensure that our destructor runs properly)
    write_inode: Some(exfat_write_inode),
    evict_inode: Some(exfat_evict_inode),
    put_super: Some(exfat_put_super),
    sync_fs: Some(exfat_sync_fs),
    statfs: Some(exfat_statfs),
    show_options: Some(exfat_show_options),

    ..unsafe { zeroed!(SuperOperations) }
};

extern "C" fn exfat_write_inode(_arg1: *mut Inode, _wbc: *mut WritebackControl) -> c_int {
    todo!("exfat_write_inode called"); // TODO
}

extern "C" fn exfat_evict_inode(inode: *mut Inode) {
    let inode = unsafe { &mut *inode }.to_info_mut();
    let sb = inode.vfs_inode.i_sb;
    let sbi = take_sb(&sb);

    unsafe { truncate_inode_pages(&mut inode.vfs_inode.i_data, 0) };

    if unsafe { inode.vfs_inode.__bindgen_anon_1.i_nlink != 0 } {
        unsafe { i_size_write(&mut inode.vfs_inode, 0) };
        let lock = sbi.state.lock();
        //__exfat_truncate(inode, 0); // TODO
        let _ = lock;
    }

    unsafe {
        invalidate_inode_buffers(&mut inode.vfs_inode);
        clear_inode(&mut inode.vfs_inode);
    }
    //exfat_cache_inval_inode(inode); // TODO
    sbi.inode_hashtable.lock().evict(inode);
}

extern "C" fn exfat_put_super(_sb: *mut SuperBlock) {
    // TODO: They do some delayed_free bs here. Find out if we can ignore
}

extern "C" fn exfat_sync_fs(sb: *mut SuperBlock, wait: c_int) -> c_int {
    from_kernel_result! {
        let sbi = take_sb(&sb);

        if wait != 0 {
            return Ok(());
        }

        let sb_state = sbi.state.lock();

        // If there are some dirty buffers in the bdev inode
        // SAFETY: TODO
        unsafe { sync_blockdev(sb_state.sb.s_bdev) };

        // TODO: clear volume dirty flag

        Ok(())
    }
}

extern "C" fn exfat_statfs(dentry: *mut Dentry, buf: *mut KStatFs) -> c_int {
    kernel::pr_info!("### Rust ExFat ### statfs\n");

    from_kernel_result! {
        let sbi = unsafe { take_sb(&(*dentry).d_sb) };
        let buf = unsafe { &mut *buf };
        let boot = &sbi.info.boot_sector_info;

        let state = sbi.state.lock();

        let bd_dev = unsafe { (*state.sb.s_bdev).bd_dev };
        let id = unsafe { huge_encode_dev(bd_dev) };

        if boot.used_clusters == EXFAT_CLUSTERS_UNTRACKED {
            // TODO: count used clusters
        }

        buf.f_type = state.sb.s_magic as i64;
        buf.f_bsize = boot.cluster_size as i64;
        buf.f_blocks = boot.num_clusters as u64 - 2;
        //buf.f_bfree = buf.f_blocks - boot.used_clusters as u64;
        buf.f_bfree = 0; // TODO
        buf.f_bavail = buf.f_bfree;
        buf.f_fsid = unsafe { u64_to_fsid(id) };

        /* Unicode utf16 255 characters */
        //buf->f_namelen = EXFAT_MAX_FILE_LEN * NLS_MAX_CHARSET_SIZE;

        Ok(())
    }
}

extern "C" fn exfat_show_options(m: *mut SeqFile, root: *mut Dentry) -> c_int {
    let sbi = unsafe { take_sb(&(*root).d_sb) };
    let options = &sbi.info.options;

    const GLOBAL_ROOT_UID: c_uint = 0;
    const GLOBAL_ROOT_GID: c_uint = 0;

    if options.fs_uid.val == GLOBAL_ROOT_UID {
        unsafe {
            seq_printf(
                m,
                b",uid=%u\0".as_ptr() as *const i8,
                //from_kuid_munged(&init_user_ns, options.fs_uid),
                options.fs_uid.val,
            );
        }
    }

    if options.fs_gid.val == GLOBAL_ROOT_GID {
        unsafe {
            seq_printf(
                m,
                b",gid=%u\0".as_ptr() as *const i8,
                //from_kgid_munged(&init_user_ns, options.fs_gid),
                options.fs_gid.val,
            );
        }
    }

    unsafe {
        seq_printf(
            m,
            b",fmask=%04o,dmask=%04o\0".as_ptr() as *const i8,
            options.fs_fmask as c_uint,
            options.fs_dmask as c_uint,
        );
    }

    if options.allow_utime != 0 {
        unsafe {
            seq_printf(
                m,
                b",allow_utime=%04o\0".as_ptr() as *const i8,
                options.allow_utime as c_uint,
            );
        }
    }

    if options.utf8 {
        unsafe {
            seq_printf(m, b",iocharset=utf8\0".as_ptr() as *const i8);
        }
        //} else if sbi.nls_io {
        //    unsafe {
        //        seq_printf(m, ",iocharset=%s".as_ptr() as *const i8, sbi.nls_io.charset);
        //    }
    }

    let errors: &[u8] = match options.errors {
        ExfatErrorMode::Continue => b",errors=continue\0",
        ExfatErrorMode::Panic => b",errors=panic\0",
        ExfatErrorMode::RemountRo => b",errors=remount-ro\0",
    };
    unsafe { seq_printf(m, errors.as_ptr() as *const i8) };

    if options.discard {
        unsafe {
            seq_printf(m, b",discard\0".as_ptr() as *const i8);
        }
    }

    if options.time_offset != 0 {
        unsafe {
            seq_printf(
                m,
                b",time_offset=%d\0".as_ptr() as *const i8,
                options.time_offset,
            );
        }
    }

    0
}
