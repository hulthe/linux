// TODO: replace this macre with a trait method
/// Read our SuperBlockInfo from the kernels super_block
#[macro_export]
macro_rules! get_exfat_sb_from_sb {
    ($x: expr) => {{
        let fs_info = $x.s_fs_info as *mut SuperBlockInfo;
        unsafe { &mut *fs_info }
    }};
}

/// Read a bdev_queue from a &mut *mut bdev
#[macro_export]
macro_rules! bdev_get_queue {
    ($bdev: expr) => {{
        let queue = unsafe { &mut **$bdev }.bd_queue;
        unsafe { &mut *queue }
    }};
}

/// Convert a block returning kernel::Result to returning c_int, useful for extern functions
#[macro_export]
macro_rules! from_kernel_result {
    ($($tt:tt)*) => {{
        match (|| {
            $($tt)*
        })() {
            kernel::Result::Ok(()) => 0,
            kernel::Result::Err(e) => e.to_kernel_errno(),
        }
    }};
}
