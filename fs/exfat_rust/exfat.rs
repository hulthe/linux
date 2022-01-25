//! A Rust implementation of the exFAT filesystem

#![no_std]

use kernel::prelude::*;

const __LOG_PREFIX: &[u8] = b"exfat_rust";

#[no_mangle]
pub extern "C" fn rust_exfat_test() {
    pr_info!("test: exFAT here!");
}
