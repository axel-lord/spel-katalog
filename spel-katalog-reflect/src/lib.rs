#![cfg_attr(not(test), no_std)]
//! Reflection utilities.

use ::core::fmt::Display;

#[doc(inline)]
pub use ::core::str::FromStr;

#[doc(inline)]
pub use ::spel_katalog_reflect_derive::{AsStr, Cycle, Fields, FromStr, Proxy, Variants};

/// Trait for simple enums to provide all values.
///
/// # Safety
/// The `VARIANTS` associated constant must contain all variants.
/// `index_of` must return the correct index.
pub unsafe trait Variants
where
    Self: 'static + Sized,
{
    /// All values for enum.
    const VARIANTS: &[Self];

    /// Get index of variant in variant array.
    fn index_of(&self) -> usize;
}

/// Trait for simple enums to cycle the value.
///
/// # Safety
/// `cycle_next` must return the cyclic next variant in `VARIANTS`.
/// `cycle_prev` must return the cyclic previous variant in `VARIANTS`.
pub unsafe trait Cycle {
    /// Get the next variant. For the last variant will return the first variant.
    fn cycle_next(&self) -> Self;

    /// Get the next variant. For the last variant will return the first variant.
    fn cycle_prev(&self) -> Self;
}

/// Trait for getting the name of an enum variant.
///
/// By default round-trips with derived [FromStr] for simple enums.
pub trait AsStr {
    /// Get the name of the current variant.
    fn as_str<'a>(&self) -> &'a str;
}

impl<T: AsStr> AsStr for &T {
    #[inline]
    fn as_str<'a>(&self) -> &'a str {
        T::as_str(self)
    }
}

/// Error returned by [FromStr] implementations
/// when trying to crate an enum from an unknown variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct UnknownVariant;

impl Display for UnknownVariant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("no variant with given name available")
    }
}
impl ::core::error::Error for UnknownVariant {}

/// Provide a proxy struct with custom non-trait methods.
pub trait Proxy {
    /// Proxy type.
    type Proxy: AsRef<Self>;

    /// Return proxy object.
    fn proxy(&self) -> &Self::Proxy;
}

/// Convert a struct to it's fields.
pub trait IntoFields {
    /// Representation of any non-skipped field.
    type Field;

    /// Collection of all non-skipped fields of self.
    type IntoFields: IntoIterator<Item = Self::Field> + AsRef<[Self::Field]>;

    /// Convert self into a collection of fields.
    fn into_fields(self) -> Self::IntoFields;

    /// Apply a single field as a change to self.
    fn delta(&mut self, delta: Self::Field);
}

/// Trait for structs providing an indexing enum to index fields.
pub trait FieldsIdx
where
    for<'this> &'this Self: IntoFields,
{
    /// Type to index fields with.
    type FieldIdx;

    /// Get a field by index.
    fn get(&self, idx: Self::FieldIdx) -> <&Self as IntoFields>::Field;
}

/// Trait for structs providing an indexing enum to index fields mutably.
pub trait FieldsIdxMut
where
    for<'this> &'this Self: IntoFields,
    for<'this> &'this mut Self: IntoFields,
    Self: FieldsIdx,
{
    /// Get a mut field by index.
    fn get_mut(&mut self, idx: Self::FieldIdx) -> <&mut Self as IntoFields>::Field;
}

/// Collection trait for all struct field access traits.
pub trait Fields
where
    for<'this> &'this Self: IntoFields,
    for<'this> &'this mut Self: IntoFields,
    Self: IntoFields + FieldsIdx + FieldsIdxMut,
{
    /// Get references to fields.
    #[inline]
    fn fields(&self) -> <&Self as IntoFields>::IntoFields {
        self.into_fields()
    }

    /// Get mutable references to fields.
    #[inline]
    fn fields_mut(&mut self) -> <&mut Self as IntoFields>::IntoFields {
        self.into_fields()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::pretty_assertions::assert_eq;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Variants, Cycle, AsStr, FromStr)]
    #[reflect(crate_path = crate, as_ref, display, try_from)]
    enum VariantsTestEnum {
        First,
        Second,
        #[as_str = "3:rd"]
        Third,
        #[as_str("4")]
        Fourth,
    }

    #[derive(Debug, Proxy, Fields)]
    #[reflect(crate_path = crate, option, getter, debug)]
    struct OptDefaultTestStruct {
        first: Option<String>,
        #[reflect(default = 5)]
        second: Option<i32>,
        #[reflect(proxy(no_option))]
        third: u32,
        #[reflect(some_pattern = Ok)]
        fourth: Result<u8, ()>,
    }

    #[test]
    fn derived_option_default_all_default() {
        let s = OptDefaultTestStruct {
            first: None,
            second: None,
            third: 7,
            fourth: Err(()),
        };

        assert_eq!(s.proxy().first().as_str(), "");
        assert_eq!(*s.proxy().second(), 5);
        assert_eq!(*s.proxy().third(), 7);
        assert_eq!(*s.proxy().fourth(), 0);
    }

    #[test]
    fn derived_option_default_all_set() {
        let s = OptDefaultTestStruct {
            first: Some(String::from("Hello")),
            second: Some(53),
            third: 9,
            fourth: Ok(15),
        };

        assert_eq!(s.proxy().first().as_str(), "Hello");
        assert_eq!(*s.proxy().second(), 53);
        assert_eq!(*s.proxy().third(), 9);
        assert_eq!(*s.proxy().fourth(), 15);
    }

    #[test]
    fn derived_variants() {
        use VariantsTestEnum::*;

        assert_eq!(First.index_of(), 0);
        assert_eq!(Fourth.index_of(), 3);

        assert_eq!(VariantsTestEnum::VARIANTS, &[First, Second, Third, Fourth]);
    }

    #[test]
    fn derived_cycle() {
        use VariantsTestEnum::*;

        assert_eq!(First.cycle_next(), Second);
        assert_eq!(First.cycle_prev(), Fourth);

        assert_eq!(Fourth.cycle_next(), First);
        assert_eq!(Fourth.cycle_prev(), Third);
    }

    #[test]
    fn derived_as_str() {
        use VariantsTestEnum::*;

        assert_eq!(First.as_str(), "First");
        assert_eq!(Second.as_str(), "Second");
    }

    #[test]
    fn derived_as_str_variants() {
        for (variant, str_rep) in VariantsTestEnum::VARIANTS
            .iter()
            .zip(["First", "Second", "3:rd", "4"])
        {
            assert_eq!(variant.as_str(), str_rep);
        }
    }

    #[test]
    fn derived_from_str() {
        use VariantsTestEnum::*;

        assert_eq!(Ok(First), "First".parse());
        assert_eq!(Ok(Second), "Second".parse());
        assert_eq!(Ok(Third), "3:rd".parse());
        assert_eq!(Ok(Fourth), "4".parse());
        assert_eq!(Err(UnknownVariant), "abc".parse::<VariantsTestEnum>());
    }
}
