use crate::external::BufferHead;
use crate::fat::ClusterIndex;
use crate::fat::FatChainReader;
use crate::superblock::{BootSectorInfo, SbState, SuperBlock, NUM_RESERVED_CLUSTERS};
use core::cmp::min;
use kernel::pr_err;
use kernel::prelude::*;

pub(crate) struct ClusterChain<'a> {
    boot: &'a BootSectorInfo,
    sb: &'a SuperBlock,

    /// The cluster index for the start of the chain
    start_cluster: ClusterIndex,

    fat_reader: FatChainReader<'a>,

    /// The current cluster
    cluster: Option<Cluster>,

    /// Enable sb_breadahead optimization
    readahead: bool,
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
            return Err(EINVAL);
        }

        Ok(ClusterChain {
            start_cluster: index,
            fat_reader: FatChainReader::new(boot, sb_state.sb, index),
            cluster: None,
            boot,
            sb: sb_state.sb,
            readahead: false,
        })
    }

    pub(crate) fn enable_readahead(self) -> Self {
        Self {
            readahead: true,
            ..self
        }
    }

    /// Get the current cluster index
    pub(crate) fn start_cluster(&self) -> ClusterIndex {
        self.start_cluster
    }

    /// Get the current cluster index.
    ///
    /// Might result in reading the next FAT entry
    pub(crate) fn index(&mut self) -> Result<Option<ClusterIndex>> {
        let cluster = match self.take_cluster()? {
            Some(cluster) => cluster,
            None => return Ok(None),
        };

        let index = cluster.index;
        self.cluster = Some(cluster);

        Ok(Some(index))
    }

    /// Skip `n` number of bytes
    pub(crate) fn skip(&mut self, n: usize) -> Result<()> {
        let mut cluster_offset = n >> self.boot.cluster_size_bits;
        let mut sector_offset =
            (n as u64 % self.boot.cluster_size as u64) >> self.sb.s_blocksize_bits;
        let mut byte_offset = n % self.sb.s_blocksize as usize;

        //pr_err!(
        //    "heap skip, n={n}, clusters={clusters}, sectors={}, bytes={}",
        //    sectors_in_cluster,
        //    bytes_in_sector
        //);

        if let Some(cluster) = self.cluster.as_ref() {
            let next_sector = cluster.sector_index + sector_offset;
            if next_sector >= self.boot.sect_per_clus as u64 {
                cluster_offset += 1;
                sector_offset = next_sector - self.boot.sect_per_clus as u64;
            }

            if let Some(sector) = cluster.sector.as_ref() {
                let next_byte = sector.byte_cursor + byte_offset;
                let block_size = self.sb.s_blocksize as usize;

                if next_byte >= block_size {
                    sector_offset += 1;
                    byte_offset = next_byte - block_size;
                }
            }
        }

        if cluster_offset > 0 {
            if self.cluster.take().is_some() && cluster_offset > 1 {
                self.fat_reader.nth(cluster_offset - 2);
            } else {
                self.fat_reader.nth(cluster_offset - 1);
            }
        }

        let mut cluster = match self.cluster.take() {
            Some(mut cluster) => {
                if sector_offset > 0 {
                    cluster.sector = None;
                    cluster.sector_index += sector_offset;
                }
                cluster
            }
            None => match self.fat_reader.next() {
                Some(Ok(next_cluster)) => Cluster {
                    index: next_cluster,
                    sector_index: sector_offset,
                    sector: None,
                },
                Some(Err(e)) => return Err(e),
                None => return Ok(()), // EOF
            },
        };

        let sector = match cluster.sector.take() {
            Some(mut sector) => {
                sector.byte_cursor += byte_offset;
                sector
            }
            None => {
                let sector =
                    cluster_to_sector(self.boot, cluster.index) + cluster.sector_index as u64;

                let data = BufferHead::block_read(self.sb, sector).ok_or(ENOMEM)?;
                if self.readahead {
                    data.readahead(self.sb);
                }

                Sector {
                    data,
                    byte_cursor: byte_offset,
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
                0 => return Err(EIO), // TODO: find a more suitable error
                n if n == buf.len() => return Ok(()),
                n => buf = &mut buf[n..],
            }
        }
    }

    fn take_cluster(&mut self) -> Result<Option<Cluster>> {
        Ok(match self.cluster.take() {
            Some(cluster) => Some(cluster),
            None => match self.fat_reader.next() {
                Some(next_cluster) => Some(Cluster {
                    index: next_cluster?,
                    sector_index: 0,
                    sector: None,
                }),
                None => None, // EOF
            },
        })
    }

    /// Read some amount of bytes from the cluster chain into `buf`
    ///
    /// Returns the number of bytes read, or `0` if everything has been read.
    pub(crate) fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // get the current cluster
        let mut cluster = match self.take_cluster()? {
            Some(cluster) => cluster,
            None => return Ok(0), // EOF
        };

        // get the current sector
        let mut sector = match cluster.sector.take() {
            Some(sector) => sector,
            None => {
                let sector = cluster_to_sector(self.boot, cluster.index) + cluster.sector_index;

                let data = BufferHead::block_read(self.sb, sector).ok_or(ENOMEM)?;

                if self.readahead {
                    data.readahead(self.sb);
                }

                Sector {
                    data,
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
