#[allow(dead_code)] // TODO
#[derive(PartialEq, Eq)]
pub(crate) enum ChecksumType {
    DirEntry = 0,
    BootSector = 1,
    Default = 2,
}

pub(crate) fn calc_checksum_16(data: &[u8], mut checksum: u16, checksum_type: ChecksumType) -> u16 {
    let calc = |byte: u8| checksum = ((checksum << 15) | (checksum >> 1)).wrapping_add(byte as u16);

    if checksum_type == ChecksumType::DirEntry {
        // skip index 2 and 3, for some reason...
        let head = &data[..2];
        let tail = &data[4..];
        head.iter().chain(tail.iter()).copied().for_each(calc);
    } else {
        data.iter().copied().for_each(calc);
    }

    checksum
}

pub(crate) fn calc_checksum_32(data: &[u8], mut checksum: u32, checksum_type: ChecksumType) -> u32 {
    let calc = |byte: u8| checksum = ((checksum << 31) | (checksum >> 1)).wrapping_add(byte as u32);

    if checksum_type == ChecksumType::BootSector {
        // Skip volume flags & percent in use fields for checksum calculation
        // (indicies 106, 107 and 112)
        data.iter()
            .copied()
            .enumerate()
            .filter(|(index, _)| ![106, 107, 112].contains(index))
            .map(|(_, b)| b)
            .for_each(calc);
    } else {
        data.iter().copied().for_each(calc);
    }

    checksum
}
