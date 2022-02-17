mod allocation_bitmap;
mod file;
mod file_name;
mod stream_extension;

use allocation_bitmap::AllocationBitmap;
use file::File;
use file_name::FileName;
use stream_extension::StreamExtension;

pub(crate) struct ToDo;

/// All possible directory entries
pub(crate) enum DirectoryEntry {
    // Critical primary
    AllocationBitmap(AllocationBitmap),
    UpCaseTable(ToDo),
    VolumeLabel(ToDo),
    File(File),

    // Benign primary
    VolumeGuid(ToDo),
    TexFatPadding(ToDo),

    // Critical secondary
    StreamExtension(StreamExtension),
    FileName(FileName),

    // Benign secondary
    VendorExtension(ToDo),
    VendorAllocation(ToDo),
}
