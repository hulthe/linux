use crate::inode::{inode_unique_num, Inode, InodeInfo};
use core::ptr::NonNull;
use kernel::bindings::{igrab, GOLDEN_RATIO_64};
use kernel::linked_list::{GetLinks, GetLinksWrapped, Links, List, Wrapper};

const HASH_TABLE_BITS: u32 = 8;
const HASH_TABLE_SIZE: usize = 1 << HASH_TABLE_BITS;

pub(crate) struct InodeHashTable {
    table: [List<InodeInfo>; HASH_TABLE_SIZE],
}

pub(crate) struct PtrWrapper<T> {
    ptr: NonNull<T>,
}

impl<T> Wrapper<T> for PtrWrapper<T> {
    fn into_pointer(self) -> NonNull<T> {
        self.ptr
    }

    unsafe fn from_pointer(ptr: NonNull<T>) -> Self {
        Self { ptr }
    }

    fn as_ref(&self) -> &T {
        unsafe { &*self.ptr.as_ptr() }
    }
}

impl<T> PtrWrapper<T> {
    pub(crate) fn wrap(t: &mut T) -> Self {
        Self {
            ptr: NonNull::from(t),
        }
    }
}

impl GetLinks for InodeInfo {
    type EntryType = Self;

    fn get_links(data: &Self) -> &Links<Self> {
        &data.inode_cache_list
    }
}

impl GetLinksWrapped for InodeInfo {
    type Wrapped = PtrWrapper<InodeInfo>;
}

#[inline]
fn hash_inode_key(key: u64) -> usize {
    let (hash, _) = key.overflowing_mul(GOLDEN_RATIO_64);
    let hash = hash >> (u64::BITS - HASH_TABLE_BITS); // high bits are more random
    hash as usize
}

impl Default for InodeHashTable {
    fn default() -> Self {
        const EMPTY: List<InodeInfo> = List::new();
        Self {
            table: [EMPTY; HASH_TABLE_SIZE],
        }
    }
}

impl InodeHashTable {
    fn bucket(&mut self, key: u64) -> &mut List<InodeInfo> {
        &mut self.table[hash_inode_key(key)]
    }

    pub(crate) fn insert(&mut self, inode: &mut InodeInfo) {
        let wrapper = PtrWrapper::wrap(inode);
        self.bucket(inode.unique_num()).push_back(wrapper);
    }

    pub(crate) fn get(
        &mut self,
        cluster_index: u32,
        dir_index: u32,
    ) -> Option<&'static mut InodeInfo> {
        // TODO: actually hash the key
        let key = inode_unique_num(cluster_index, dir_index);

        let bucket = self.bucket(key);

        let mut cursor = bucket.cursor_front();

        while let Some(inode) = cursor.current() {
            cursor.move_next();

            if key == inode.unique_num() {
                let inode = unsafe { igrab(inode as *const _ as *mut Inode) };
                let inode = inode as *mut InodeInfo;

                // if igrab returns null, the C version just continues the loop.
                // that doesn't seem like the correct behaviour.
                // TODO: figure out if we can return regardless.
                // return unsafe { inode.as_mut() };
                if let Some(inode) = unsafe { inode.as_mut() } {
                    return Some(inode);
                }
            }
        }

        None
    }

    pub(crate) fn evict(&mut self, inode: &mut InodeInfo) {
        let wrapper = PtrWrapper::wrap(inode);
        let key = inode.unique_num();
        // SAFETY: TODO
        unsafe { self.bucket(key).remove(&wrapper) };
    }
}
