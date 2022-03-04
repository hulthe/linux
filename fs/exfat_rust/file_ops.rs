use crate::directory::DirEntryReader;
use crate::from_kernel_result;
use crate::inode::InodeExt;
use crate::superblock::take_sb;
use core::ptr::null_mut;
use kernel::bindings::{
    __generic_file_fsync, blkdev_issue_flush, dir_context as DirContext, dir_emit_dots,
    file as File, file_operations as FileOperations, generic_file_llseek, generic_read_dir, loff_t,
    sync_blockdev,
};
use kernel::c_types::c_int;
use kernel::pr_info;

pub(crate) static mut DIR_OPERATIONS: FileOperations = FileOperations {
    llseek: Some(generic_file_llseek),
    read: Some(generic_read_dir),
    iterate: Some(exfat_iterate),
    unlocked_ioctl: None, // TODO
    compat_ioctl: None,   // TODO
    fsync: Some(file_fsync),

    // Should be none
    owner: null_mut(),
    write: None,
    read_iter: None,
    write_iter: None,
    iopoll: None,
    iterate_shared: None,
    poll: None,
    mmap: None,
    mmap_supported_flags: 0,
    open: None,
    flush: None,
    release: None,
    fasync: None,
    lock: None,
    sendpage: None,
    get_unmapped_area: None,
    check_flags: None,
    flock: None,
    splice_write: None,
    splice_read: None,
    setlease: None,
    fallocate: None,
    show_fdinfo: None,
    copy_file_range: None,
    remap_file_range: None,
    fadvise: None,
};

extern "C" fn file_fsync(file: *mut File, start: loff_t, end: loff_t, datasync: c_int) -> c_int {
    pr_info!("file_fsync called");
    let inode = unsafe { (*(*file).f_mapping).host };

    let err = unsafe { __generic_file_fsync(file, start, end, datasync) };
    if err != 0 {
        return err;
    }

    let block_device = unsafe { (*(*inode).i_sb).s_bdev };
    let err = unsafe { sync_blockdev(block_device) };
    if err != 0 {
        return err;
    }

    return unsafe { blkdev_issue_flush(block_device) };
}

extern "C" fn exfat_iterate(file: *mut File, context: *mut DirContext) -> c_int {
    pr_info!("exfat_iterate called");
    from_kernel_result! {
        const ITER_POS_FILLED_DOTS: u64 = 2;

        let context = unsafe { &mut *context };
        let file = unsafe { &mut *file };
        let dentry = unsafe { &*file.f_path.dentry };
        let inode = unsafe { &*dentry.d_inode };
        let inode = inode.to_info();

        let sbi = take_sb(&inode.vfs_inode.i_sb);
        let sb_info = &sbi.info;
        let sb_lock = sbi.state.as_ref().unwrap();
        let sb_state = sb_lock.lock();

        if unsafe { !dir_emit_dots(file, context) } {
            return Ok(())
        }


        pr_info!("exfat_iterate start_cluster {}", inode.start_cluster);
        let mut sb_lock_guard = Some(sb_state);
        loop {
            let sb_state = sb_lock_guard
                .take()
                .unwrap_or_else(|| sb_lock.lock());

            let entry_index = context.pos as u64 - ITER_POS_FILLED_DOTS;
            context.pos += 1;
            pr_info!("exfat_iterate next entry {entry_index}");

            let reader = DirEntryReader::new(sb_info, &sb_state, inode.start_cluster)?;
            let mut reader = reader.skip(entry_index as usize);

            let dir_entry = match reader.next() {
                None => break,
                Some(entry) => entry?,
            };

            pr_info!("exfat_iterate reading entry {dir_entry:?}");

            // dir_emit() can trigger a page fault, therefore we should drop the lock before
            // calling it

            // TODO: exfat_iget to aquire an inode

            let _ = sb_state;

            // TODO: dir_emit
        }

        Ok(())
    }
}
