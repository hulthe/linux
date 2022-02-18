use crate::external::BufferHead;
use crate::fat::ClusterIndex;
use crate::fat::FatChainReader;
use crate::superblock::SuperBlockInfo;
use core::cmp::min;
use kernel::bindings::super_block;
use kernel::{Error, Result};

pub(crate) struct ClusterChain<'a> {
    state: Option<ClusterChainState<'a>>,
}

struct ClusterChainState<'a> {
    sb: &'a super_block,
    sbi: &'a SuperBlockInfo,
    fat_reader: FatChainReader<'a>,
    current_cluster: ClusterIndex,
    sector: BufferHead,
    sector_cursor: usize,
    cluster_sector: u64,
}

impl<'a> ClusterChain<'a> {
    /// Read some amount of bytes from the cluster chain into `buf`
    ///
    /// Returns the number of bytes read, or `0` if everything has been read.
    pub(crate) fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return Ok(0),
        };

        let load_sector = |state: &ClusterChainState<'a>| {
            let bs = &state.sbi.boot_sector_info;
            let sector = (state.current_cluster - 2) as u64 * bs.sect_per_clus as u64
                + bs.data_start_sector
                + state.cluster_sector;

            BufferHead::block_read(state.sb, sector).ok_or(Error::ENOMEM)
        };

        let bytes = &state.sector.bytes()[state.sector_cursor..];
        let write_len = min(buf.len(), bytes.len());

        buf[..write_len].copy_from_slice(&bytes[..write_len]);
        state.sector_cursor += write_len;

        if state.sector_cursor as u64 == state.sb.s_blocksize {
            // finished reading sector

            state.sector_cursor = 0;
            state.cluster_sector += 1;

            if state.cluster_sector == state.sbi.boot_sector_info.sect_per_clus as u64 {
                // finished reading cluster
                state.cluster_sector = 0;
                match state.fat_reader.next() {
                    Some(Ok(next_cluster)) => {
                        state.current_cluster = next_cluster;
                        state.sector = load_sector(&state)?;
                    }
                    Some(Err(e)) => {
                        self.state = None;
                        return Err(e);
                    }
                    None => {
                        self.state = None;
                    }
                }
            } else {
                // next sector in cluster
                state.sector = load_sector(&state)?;
            }
        }

        return Ok(write_len);
    }
}
