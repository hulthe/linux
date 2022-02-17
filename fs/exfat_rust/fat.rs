pub(crate) type ClusterIndex = u32;

/// One entry in the FAT
#[derive(Debug)]
pub(crate) enum FatEntry {
    /// The corresponding cluster is bad
    Bad,

    /// The corresponding cluster the last of a cluster chain
    LastOfChain,

    /// This points to the *next* FatEntry in the given cluster chain.
    ///
    /// This must not point to a FatEntry that preceeds it
    NextFat(ClusterIndex),
}
