use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::mem::{align_of, size_of};
use core::ptr::NonNull;
use kernel::bindings::{
    self, gfp_t, kmem_cache_alloc, kmem_cache_create, kmem_cache_destroy, kmem_cache_free,
    slab_flags_t,
};
use kernel::c_types::{c_char, c_uint, c_void};
use kernel::str::CStr;
use kernel::{pr_err, Error, Result};

pub(crate) struct KMemCache<T> {
    _phantom: PhantomData<T>,
    cache: UnsafeCell<Option<NonNull<bindings::kmem_cache>>>,
}

// SAFETY: TODO
unsafe impl<T> Sync for KMemCache<T> {}

impl<T> KMemCache<T> {
    pub(crate) const fn new() -> Self {
        Self {
            _phantom: PhantomData,
            cache: UnsafeCell::new(None),
        }
    }
}

impl<T: PtrInit> KMemCache<T> {
    extern "C" fn object_constructor(ptr: *mut c_void) {
        let ptr = ptr as *mut T;

        // SAFETY: kmem will make sure that the pointer points to a valid location
        let ptr = NonNull::new(ptr).expect("pointer is not null"); // TODO

        T::init_ptr(ptr);
    }

    /// # SAFETY
    ///
    /// This function must be called exactly once, before any call to alloc() is made
    pub(crate) unsafe fn create(&self, name: &str, flags: slab_flags_t) -> Result {
        kernel::pr_info!("KMemCache::create called");

        let name = CStr::from_bytes_with_nul(name.as_bytes())?.as_ptr();
        let align = align_of::<T>() as c_uint;
        let size = size_of::<T>() as c_uint;

        let cache = unsafe {
            kmem_cache_create(
                name as *const c_char,
                size,
                align,
                flags,
                Some(Self::object_constructor),
            )
        };
        let cache = Some(NonNull::new(cache).ok_or(Error::ENOMEM)?);
        unsafe { *self.cache.get() = cache }

        Ok(())
    }

    /// # SAFETY
    ///
    /// This function must be called after all references to objects in the cache have been dropped
    pub(crate) unsafe fn destroy(&self) {
        let cache = match self.get_cache() {
            Ok(cache) => cache,
            Err(_) => return,
        };

        unsafe { kmem_cache_destroy(cache) };
    }

    fn get_cache(&self) -> Result<*mut bindings::kmem_cache> {
        // SAFETY: The caller has to make sure to call create() before anything else, making sure
        // self.cache has been initialized with a valid pointer. After that, only immutable
        // references te self.cache is aquired.
        let cache = unsafe { &*self.cache.get() };
        cache
            .ok_or_else(|| {
                pr_err!("kmem_cache was not properly initialized before use");
                Error::EINVAL
            })
            .map(|non_null| non_null.as_ptr())
    }

    pub(crate) fn alloc(&self, flags: gfp_t) -> Result<NonNull<T>> {
        let cache = self.get_cache()?;

        // SAFETY: self.cache was Some, therefore the cache was
        // properly initialized by a call to self.crate()
        let ptr = unsafe { kmem_cache_alloc(cache, flags) as *mut T };

        NonNull::new(ptr).ok_or(Error::ENOMEM)
    }

    pub(crate) fn free(&self, object: NonNull<T>) {
        let cache = match self.get_cache() {
            Ok(cache) => cache,
            Err(_) => return,
        };

        // SAFETY: self.cache was Some, therefore the cache was
        // properly initialized by a call to self.crate()
        unsafe { kmem_cache_free(cache, object.as_ptr() as *mut c_void) };
    }
}

pub(crate) trait PtrInit {
    fn init_ptr(ptr: NonNull<Self>);
}
