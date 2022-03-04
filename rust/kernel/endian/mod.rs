//! This module contains variants of primitive types with a platform-independent little- or
//! bit-endian representation

use core::fmt::{self, Debug};

/// Define conversions to/from this type in its big/little-endian form
pub trait SpecificEndian
where
    Self: Clone + Copy,
{
    /// Convert this CPU native value to its big-endian form
    fn to_big_endian(&self) -> Self;

    /// Convert this CPU native value to its little-endian form
    fn to_little_endian(&self) -> Self;

    /// Convert the big-endian form of this value to its CPU native form
    fn from_big_endian(&self) -> Self;

    /// Convert the little-endian form of this value to its CPU native form
    fn from_little_endian(&self) -> Self;
}

macro_rules! impl_specific_endian_for_primitive {
    ($wrap_ty:ty) => {
        impl SpecificEndian for $wrap_ty {
            #[inline]
            fn to_big_endian(&self) -> Self {
                self.to_be()
            }

            #[inline]
            fn to_little_endian(&self) -> Self {
                self.to_le()
            }

            #[inline]
            fn from_big_endian(&self) -> Self {
                Self::from_be(*self)
            }

            #[inline]
            fn from_little_endian(&self) -> Self {
                Self::from_le(*self)
            }
        }
    };
}

impl_specific_endian_for_primitive!(u16);
impl_specific_endian_for_primitive!(i16);
impl_specific_endian_for_primitive!(u32);
impl_specific_endian_for_primitive!(i32);
impl_specific_endian_for_primitive!(u64);
impl_specific_endian_for_primitive!(i64);
impl_specific_endian_for_primitive!(u128);
impl_specific_endian_for_primitive!(i128);
impl_specific_endian_for_primitive!(usize);
impl_specific_endian_for_primitive!(isize);

/// A little-endian representation of T.
///
/// Use `::from()` or `.into()` to convert between this and the native representation of T
#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct LittleEndian<T>
where
    T: SpecificEndian + Copy + Clone,
{
    /// The raw little-endian representation of the value
    v: T,
}

impl<T> LittleEndian<T>
where
    T: SpecificEndian,
{
    /// Returns the raw data stored in the struct.
    #[inline]
    pub fn to_bits(&self) -> T {
        self.v
    }

    /// Imports the data raw into a LittleEndian<T> struct.
    #[inline]
    pub fn from_bits(v: T) -> Self {
        Self { v }
    }

    /// Converts the data to the same type T in host-native endian.
    #[inline]
    pub fn to_native(&self) -> T {
        T::from_little_endian(&self.v)
    }
}

impl<T> Debug for LittleEndian<T>
where
    T: SpecificEndian + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}_le", self.to_native())
    }
}

impl<T: SpecificEndian> From<T> for LittleEndian<T> {
    fn from(v: T) -> LittleEndian<T> {
        LittleEndian {
            v: v.to_little_endian(),
        }
    }
}

/// A macro for implementing LittleEndian for types that also implement SpecificEndian
macro_rules! make_primitive_type_from_le {
    ($wrap_ty:ty) => {
        impl From<LittleEndian<$wrap_ty>> for $wrap_ty {
            fn from(v: LittleEndian<$wrap_ty>) -> $wrap_ty {
                v.v.from_little_endian()
            }
        }
    };
}

make_primitive_type_from_le!(u16);
make_primitive_type_from_le!(i16);
make_primitive_type_from_le!(u32);
make_primitive_type_from_le!(i32);
make_primitive_type_from_le!(u64);
make_primitive_type_from_le!(i64);
make_primitive_type_from_le!(u128);
make_primitive_type_from_le!(i128);
make_primitive_type_from_le!(usize);
make_primitive_type_from_le!(isize);

mod primitive_aliases {
    #![allow(non_camel_case_types, missing_docs)]

    use super::LittleEndian;

    pub type u16le = LittleEndian<u16>;
    pub type i16le = LittleEndian<i16>;
    pub type u32le = LittleEndian<u32>;
    pub type i32le = LittleEndian<i32>;
    pub type u64le = LittleEndian<u64>;
    pub type i64le = LittleEndian<i64>;
    pub type u128le = LittleEndian<u128>;
    pub type i128le = LittleEndian<i128>;
    pub type usizele = LittleEndian<usize>;
    pub type isizele = LittleEndian<isize>;
}

pub use primitive_aliases::*;
