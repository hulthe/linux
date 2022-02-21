use alloc::vec::Vec;
use kernel::bindings::{inode as Inode};

const EXFAT_HASH_BITS: usize = 8;
const EXFAT_HASH_SIZE: usize = 1 << EXFAT_HASH_BITS;

#[allow(dead_code)]
pub(crate) struct InodeHashTable {
    inner: [Vec<Inode>; EXFAT_HASH_SIZE]
}

impl InodeHashTable {
    pub(crate) fn new() -> Self {
        const EMPTY: Vec<Inode> = Vec::new();
        Self {
            inner: [EMPTY; EXFAT_HASH_SIZE]
        }
    }
}