use crate::external::BufferHead;
use crate::fat::ClusterIndex;
use crate::fat::FatChainReader;
use crate::superblock::{SbInfo, SbState, SuperBlock, NUM_RESERVED_CLUSTERS};
use core::cmp::min;
use kernel::{pr_err, Error, Result};

pub(crate) struct ClusterChain<'a> {
    state: Option<ClusterChainState<'a>>,
}

struct ClusterChainState<'a> {
    sb_info: &'a SbInfo,
    sb: &'a SuperBlock,

    fat_reader: FatChainReader<'a>,

    /// The current cluster index in the chain
    current_cluster: ClusterIndex,

    /// The current sector
    sector: BufferHead,

    /// The current byte within the current sector, from 0
    sector_cursor: usize,

    /// The current sector within the current cluster, from 0
    cluster_sector: u64,
}

pub(crate) fn cluster_to_sector(sb_info: &SbInfo, cluster: ClusterIndex) -> u64 {
    let bs = &sb_info.boot_sector_info;
    (cluster - 2) as u64 * bs.sect_per_clus as u64 + bs.data_start_sector
}

impl<'a> ClusterChain<'a> {
    pub(crate) fn new(
        sb_info: &'a SbInfo,
        sb_state: &'a SbState<'a>,
        index: ClusterIndex,
    ) -> Result<Self> {
        if !(NUM_RESERVED_CLUSTERS..sb_info.boot_sector_info.cluster_count()).contains(&index) {
            pr_err!("Tried to read invalid cluster index ({index})");
            return Err(Error::EINVAL);
        }

        let start_sector = cluster_to_sector(sb_info, index);
        let state = ClusterChainState {
            sector: BufferHead::block_read(sb_state.sb, start_sector).ok_or(Error::EIO)?,
            sector_cursor: 0,
            cluster_sector: 0,
            current_cluster: index,
            fat_reader: FatChainReader::new(sb_state.sb, index),
            sb_info,
            sb: sb_state.sb,
        };

        Ok(ClusterChain { state: Some(state) })
    }

    /// Read the exact amount of bytes to fill `buf`.
    pub(crate) fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        let mut buf = buf;
        loop {
            match self.read(buf)? {
                0 => return Err(Error::EIO), // TODO: find a more suitable error
                n if n == buf.len() => return Ok(()),
                n => buf = &mut buf[n..],
            }
        }
    }

    /// Read some amount of bytes from the cluster chain into `buf`
    ///
    /// Returns the number of bytes read, or `0` if everything has been read.
    pub(crate) fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return Ok(0),
        };

        let load_sector = |state: &ClusterChainState<'a>| {
            let sector =
                cluster_to_sector(state.sb_info, state.current_cluster) + state.cluster_sector;
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

            if state.cluster_sector == state.sb_info.boot_sector_info.sect_per_clus as u64 {
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
