use crate::external::BufferHead;
use crate::fat::ClusterIndex;
use crate::fat::FatChainReader;
use crate::superblock::{BootSectorInfo, SbState, SuperBlock, NUM_RESERVED_CLUSTERS};
use core::cmp::min;
use kernel::{pr_err, Error, Result};

pub(crate) struct ClusterChain<'a> {
    boot: &'a BootSectorInfo,
    sb: &'a SuperBlock,

    /// The cluster index for the start of the chain
    start_cluster: ClusterIndex,

    fat_reader: FatChainReader<'a>,

    /// The current cluster
    cluster: Option<Cluster>,
}

struct Cluster {
    /// The index of the cluster
    index: ClusterIndex,

    /// The current relative sector within the cluster
    sector_index: u64,

    /// The current sector
    sector: Option<Sector>,
}

struct Sector {
    /// The sector data
    data: BufferHead,

    /// The current byte within the sector, start at 0
    byte_cursor: usize,
}

pub(crate) fn cluster_to_sector(boot: &BootSectorInfo, cluster: ClusterIndex) -> u64 {
    (cluster - 2) as u64 * boot.sect_per_clus as u64 + boot.data_start_sector
}

impl<'a> ClusterChain<'a> {
    pub(crate) fn new(
        boot: &'a BootSectorInfo,
        sb_state: &'a SbState<'a>,
        index: ClusterIndex,
    ) -> Result<Self> {
        let cluster_count = boot.cluster_count();
        let cluster_end = cluster_count + NUM_RESERVED_CLUSTERS;
        if !(NUM_RESERVED_CLUSTERS..cluster_end).contains(&index) {
            pr_err!("Tried to read invalid cluster index: 0x{index:x}");
            return Err(Error::EINVAL);
        }

        Ok(ClusterChain {
            start_cluster: index,
            fat_reader: FatChainReader::new(boot, sb_state.sb, index),
            cluster: None,
            boot,
            sb: sb_state.sb,
        })
    }

    /// Get the current cluster index
    pub(crate) fn start_cluster(&self) -> u32 {
        self.start_cluster
    }

    /// Skip `n` number of bytes
    pub(crate) fn skip(&mut self, n: usize) -> Result<()> {
        let clusters = n / self.boot.cluster_size as usize;

        let sectors_in_cluster =
            (n % self.boot.cluster_size as usize) / self.sb.s_blocksize as usize;

        let bytes_in_sector = n % self.sb.s_blocksize as usize;

        //pr_err!(
        //    "heap skip, n={n}, clusters={clusters}, sectors={}, bytes={}",
        //    sectors_in_cluster,
        //    bytes_in_sector
        //);

        let mut clusters_to_skip = clusters;
        let mut sectors_to_skip = sectors_in_cluster as u64;
        let mut bytes_to_skip = bytes_in_sector;

        if let Some(cluster) = self.cluster.as_ref() {
            let next_sector = cluster.sector_index + sectors_to_skip;
            if next_sector >= self.boot.sect_per_clus as u64 {
                clusters_to_skip += 1;
                sectors_to_skip = next_sector - self.boot.sect_per_clus as u64;
            }

            if let Some(sector) = cluster.sector.as_ref() {
                let next_byte = sector.byte_cursor + bytes_in_sector;
                let block_size = self.sb.s_blocksize as usize;
                if next_byte >= block_size {
                    sectors_to_skip += 1;
                    bytes_to_skip = next_byte - block_size;
                }
            }
        }

        if clusters_to_skip > 0 {
            self.cluster = None;
            if clusters_to_skip > 1 {
                self.fat_reader.nth(clusters_to_skip - 2);
            }
        }

        let mut cluster = match self.cluster.take() {
            Some(mut cluster) => {
                if sectors_to_skip > 0 {
                    cluster.sector = None;
                    cluster.sector_index += sectors_to_skip;
                }
                cluster
            }
            None => match self.fat_reader.next() {
                Some(Ok(next_cluster)) => Cluster {
                    index: next_cluster,
                    sector_index: sectors_to_skip,
                    sector: None,
                },
                Some(Err(e)) => return Err(e),
                None => return Ok(()), // EOF
            },
        };

        let sector = match cluster.sector.take() {
            Some(mut sector) => {
                sector.byte_cursor += bytes_to_skip;
                sector
            }
            None => {
                let sector =
                    cluster_to_sector(self.boot, cluster.index) + cluster.sector_index as u64;
                Sector {
                    data: BufferHead::block_read(self.sb, sector).ok_or(Error::ENOMEM)?,
                    byte_cursor: bytes_to_skip,
                }
            }
        };

        cluster.sector = Some(sector);
        self.cluster = Some(cluster);

        Ok(())
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
        // get the current cluster
        let mut cluster = match self.cluster.take() {
            Some(cluster) => cluster,
            None => match self.fat_reader.next() {
                Some(Ok(next_cluster)) => Cluster {
                    index: next_cluster,
                    sector_index: 0,
                    sector: None,
                },
                Some(Err(e)) => return Err(e),
                None => return Ok(0), // EOF
            },
        };

        // get the current sector
        let mut sector = match cluster.sector.take() {
            Some(sector) => sector,
            None => {
                let sector = cluster_to_sector(self.boot, cluster.index) + cluster.sector_index;

                Sector {
                    data: BufferHead::block_read(self.sb, sector).ok_or(Error::ENOMEM)?,
                    byte_cursor: 0,
                }
            }
        };

        // copy bytes
        let bytes = &sector.data.bytes()[sector.byte_cursor..];
        let write_len = min(buf.len(), bytes.len());
        buf[..write_len].copy_from_slice(&bytes[..write_len]);
        sector.byte_cursor += write_len;

        // check if we've not yet read the entire sector
        if (sector.byte_cursor as u64) < self.sb.s_blocksize {
            // ff so, keep reading the sector next time
            cluster.sector = Some(sector);
            self.cluster = Some(cluster);
        } else {
            // else move to the next sector in the cluster
            cluster.sector_index += 1;

            // check if we've not yet read all sectors in the cluster
            if cluster.sector_index < self.boot.sect_per_clus as u64 {
                // if so, we keep reading the same cluster next time
                self.cluster = Some(cluster);
            }
        }

        return Ok(write_len);
    }
}
