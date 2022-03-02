use crate::directory::DirEntryReader;
use crate::from_kernel_result;
use crate::inode::InodeExt;
use crate::superblock::take_sb;
use core::ptr::null_mut;
use kernel::bindings::{
    dir_context as DirContext, dir_emit_dots, file as File, file_operations as FileOperations,
    generic_file_llseek, generic_read_dir,
};
use kernel::c_types::c_int;

// TODO: create and export `file_operations`-struct

pub(crate) static mut DIR_OPERATIONS: FileOperations = FileOperations {
    owner: null_mut(),
    llseek: Some(generic_file_llseek),
    read: Some(generic_read_dir),
    iterate: Some(exfat_iterate),
    unlocked_ioctl: None, // TODO
    compat_ioctl: None,   // TODO
    fsync: None,          // TODO

    // Should be none
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

extern "C" fn exfat_iterate(file: *mut File, context: *mut DirContext) -> c_int {
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


        let mut sb_lock_guard = Some(sb_state);
        loop {
            let sb_state = sb_lock_guard
                .take()
                .unwrap_or_else(|| sb_lock.lock());

            let entry_index = context.pos as u64 - ITER_POS_FILLED_DOTS;
            context.pos += 1;

            let reader = DirEntryReader::new(sb_info, &sb_state, inode.start_cluster)?;
            let mut reader = reader.skip(entry_index as usize);

            let dir_entry = match reader.next() {
                None => break,
                Some(entry) => entry?,
            };

            // dir_emit() can trigger a page fault, therefore we should drop the lock before
            // calling it

            // TODO: exfat_iget to aquire an inode

            let _ = sb_state;

            // TODO: dir_emit
        }

        Ok(())
    }
}
