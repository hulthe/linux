//! stuff that should probably be moved to kernel lib

use kernel::bindings::{__bread_gfp, buffer_head, sector_t, super_block, ___GFP_MOVABLE};
use kernel::c_types::c_uint;

#[inline(always)]
pub(crate) fn sb_bread(sb: &mut super_block, block: sector_t) -> Option<&mut buffer_head> {
    unsafe {
        // TODO: is this the right ___GFP_MOVABLE? (two vs three underscores)
        // SAFEY: i have no idea
        __bread_gfp(sb.s_bdev, block, sb.s_blocksize as c_uint, ___GFP_MOVABLE).as_mut()
    }
}
