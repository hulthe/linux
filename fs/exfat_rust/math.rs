use kernel::bindings::timespec64 as TimeSpec64;

/// Rounds a number up to the next multiple of the given base.
/// # Arguments
///
/// * `num` - A number to round
/// * `base` - The base from which to base the multiple. !!NOTE!! MUST BE A POWER OF 2.
#[inline(always)]
pub(crate) const fn round_up_to_next_multiple(num: u64, base: u64) -> u64 {
    (num - 1) | (base - 1) + 1
}

/// Rounds a number down to the previous multiple of the given base.
/// # Arguments
///
/// * `num` - A number to round
/// * `base` - The base from which to base the multiple. !!NOTE!! MUST BE A POWER OF 2.
#[inline(always)]
pub(crate) const fn round_down_to_prev_multiple(num: u64, base: u64) -> u64 {
    num & (!(base - 1))
}

/// TODO: Should this method be here or in another file?
/// Truncates the access time value for the exfat context
/// C name: `exfat_truncate_atime`
pub(crate) fn truncate_atime(ts: &mut TimeSpec64) {
    ts.tv_sec = round_down_to_prev_multiple(ts.tv_sec as u64, 2) as i64;
    ts.tv_nsec = 0;
}
