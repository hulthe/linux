use kernel::Result;

/// Fold operator for counting the number of Oks in an iterator of results
pub(crate) fn count_oks<T>(bucket: Result<u32>, item: Result<T>) -> Result<u32> {
    let _ = item?;
    Ok(bucket? + 1)
}
