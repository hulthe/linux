//! stuff that should probably be moved to kernel lib

use core::slice;
use kernel::bindings::{__bread_gfp, __brelse, buffer_head, sector_t, super_block, ___GFP_MOVABLE};
use kernel::c_types::c_uint;

pub(crate) struct BufferHead {
    sector: sector_t,
    ptr: *mut buffer_head,
}

impl BufferHead {
    pub(crate) fn block_read(sb: &super_block, sector: sector_t) -> Option<Self> {
        let ptr = unsafe {
            // TODO: is this the right ___GFP_MOVABLE? (two vs three underscores)
            // SAFETY: i have no idea
            __bread_gfp(sb.s_bdev, sector, sb.s_blocksize as c_uint, ___GFP_MOVABLE).as_mut()?
        };

        Some(BufferHead { sector, ptr })
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        unsafe {
            let bh = &*self.ptr;
            slice::from_raw_parts(bh.b_data as *const u8, bh.b_size)
        }
    }

    pub(crate) fn raw_bytes(&self) -> *const u8 {
        self.bytes().as_ptr()
    }

    pub(crate) fn sector(&self) -> sector_t {
        self.sector
    }
}

impl Drop for BufferHead {
    fn drop(&mut self) {
        // Try to free the buffer
        unsafe { __brelse(self.ptr) }
    }
}
