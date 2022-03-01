use crate::directory::DirEntryReader;
use crate::from_kernel_result;
use crate::inode::{Inode, InodeExt};
use crate::superblock::take_sb;
use kernel::bindings::{dir_context as DirContext, dir_emit, dir_emit_dots, file as File};
use kernel::c_types::c_int;

// TODO: create and export `file_operations`-struct

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
        let sb_state = sbi.state.as_ref().unwrap().lock();

        if unsafe { !dir_emit_dots(file, context) } {
            return Ok(())
        }

        let mut entry_index = context.pos as u64 - ITER_POS_FILLED_DOTS;


        // TODO: read dir entry nr. context.cpos - 2
        let reader = DirEntryReader::new(sb_info, &sb_state, inode.start_cluster);




        Ok(())
    }
}
