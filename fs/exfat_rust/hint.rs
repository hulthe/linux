use core::num::NonZeroU32;

#[derive(Clone, Copy)]
pub(crate) struct ClusterHint {
    /// The absolute cluster index
    pub(crate) index: NonZeroU32,

    /// The cluster index within the chain
    pub(crate) offset: u32,
}

impl ClusterHint {
    pub(crate) fn new(index: u32, offset: u32) -> Option<Self> {
        NonZeroU32::new(index).map(|index| Self { index, offset })
    }
}
