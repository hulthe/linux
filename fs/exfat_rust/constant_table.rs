use kernel::c_types;

/// Table entry to map between kernel "constants" and our "constants"
#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct ConstantTable {
    /// The kernel's name of the constant
    pub(crate) name: *const c_types::c_char,

    /// Our value used to represent the constant
    pub(crate) value: c_types::c_int,
}

unsafe impl Send for ConstantTable {}
unsafe impl Sync for ConstantTable {}