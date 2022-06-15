use core::ops::Deref;
use core::ptr::NonNull;
use kernel::bindings::{
    kmem_cache_alloc, kmem_cache_free, names_cachep, PATH_MAX, ___GFP_DIRECT_RECLAIM, ___GFP_FS,
    ___GFP_IO, ___GFP_KSWAPD_RECLAIM,
};
use kernel::prelude::*;

const BUF_LEN: usize = PATH_MAX as usize;

// TODO
//#define __putname(name)		kmem_cache_free(names_cachep, (void *)(name))

pub(crate) struct PathStr {
    ptr: NonNull<[u8; BUF_LEN]>,
    len: usize,
}

impl PathStr {
    pub(crate) fn new() -> Result<Self> {
        const GFP_KERNEL: u32 =
            ___GFP_DIRECT_RECLAIM | ___GFP_KSWAPD_RECLAIM | ___GFP_IO | ___GFP_FS;

        let ptr = NonNull::new(unsafe { kmem_cache_alloc(names_cachep, GFP_KERNEL) })
            .ok_or_else(|| {
                pr_err!("Failed to allocate namebuffer");
                ENOMEM
            })?
            .cast();

        Ok(PathStr { ptr, len: 0 })
    }

    fn mut_slice(&mut self) -> &mut [u8; BUF_LEN] {
        // SAFETY: Pointer is protected by self, no one else has this pointer
        unsafe { &mut *self.ptr.as_ptr() }
    }

    fn slice(&self) -> &[u8; BUF_LEN] {
        // SAFETY: Pointer is protected by self, no one else has this pointer
        unsafe { &*self.ptr.as_ptr() }
    }

    pub(crate) fn push(&mut self, c: char) -> Result<()> {
        let mut utf8_buf = [0u8; 4];
        let encoded = c.encode_utf8(&mut utf8_buf);
        self.push_str(encoded)
    }

    pub(crate) fn push_str(&mut self, s: &str) -> Result<()> {
        let (start, end) = (self.len, self.len + s.len());
        let slice = self.mut_slice();
        if end > slice.len() {
            return Err(ENOMEM);
        }

        slice[start..end].copy_from_slice(s.as_bytes());
        self.len += s.len();

        Ok(())
    }
}

impl Drop for PathStr {
    fn drop(&mut self) {
        unsafe { kmem_cache_free(names_cachep, self.ptr.as_ptr().cast()) };
    }
}

impl Deref for PathStr {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        // SAFETY: buffer is only filled from str:s
        unsafe { core::str::from_utf8_unchecked(&self.slice()[..self.len]) }
    }
}
