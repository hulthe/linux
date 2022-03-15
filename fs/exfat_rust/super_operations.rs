use crate::inode::alloc_inode;
use crate::zeroed;
use kernel::bindings::{
    dentry as Dentry, inode as Inode, kstatfs as KStatFs, seq_file as SeqFile,
    super_block as SuperBlock, super_operations as SuperOperations,
    writeback_control as WritebackControl,
};
use kernel::c_types::c_int;

pub(crate) static mut EXFAT_SOPS: SuperOperations = SuperOperations {
    alloc_inode: Some(alloc_inode),
    free_inode: Some(exfat_free_inode),
    destroy_inode: Some(exfat_destroy_inode), // (Not in C version but we'll need it to ensure that our destructor runs properly)
    write_inode: Some(exfat_write_inode),
    evict_inode: Some(exfat_evict_inode),
    put_super: Some(exfat_put_super),
    sync_fs: Some(exfat_sync_fs),
    statfs: Some(exfat_statfs),
    show_options: Some(exfat_show_options),

    ..unsafe { zeroed!(SuperOperations) }
};

extern "C" fn exfat_free_inode(_inode: *mut Inode) {
    todo!("exfat_free_inode called"); // TODO
}

extern "C" fn exfat_destroy_inode(_inode: *mut Inode) {
    todo!("exfat_free_inode called"); // TODO
}

extern "C" fn exfat_write_inode(_arg1: *mut Inode, _wbc: *mut WritebackControl) -> c_int {
    todo!("exfat_write_inode called"); // TODO
}

extern "C" fn exfat_evict_inode(_arg1: *mut Inode) {
    todo!("exfat_evict_inode called"); // TODO
}

extern "C" fn exfat_put_super(_arg1: *mut SuperBlock) {
    todo!("exfat_put_super called"); // TODO
}

extern "C" fn exfat_sync_fs(_sb: *mut SuperBlock, _wait: c_int) -> c_int {
    todo!("exfat_sync_fs called"); // TODO
}

extern "C" fn exfat_statfs(_arg1: *mut Dentry, _arg2: *mut KStatFs) -> c_int {
    todo!("exfat_statfs called"); // TODO
}

extern "C" fn exfat_show_options(_arg1: *mut SeqFile, _arg2: *mut Dentry) -> c_int {
    todo!("exfat_show_options called"); // TODO
}
