#![no_std]

//! Reflection utilities.

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
pub unsafe trait Cycle
where
    Self: Variants,
{
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

#[doc(inline)]
pub use ::spel_katalog_reflect_derive::{AsStr, Cycle, Variants};

#[doc(inline)]
pub use ::core::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;
    use ::pretty_assertions::assert_eq;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Variants, Cycle, AsStr)]
    #[reflect(crate_path = crate)]
    enum VariantsTestEnum {
        First,
        Second,
        Third,
        Fourth,
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
            .zip(["First", "Second", "Third", "Fourth"])
        {
            assert_eq!(variant.as_str(), str_rep);
        }
    }
}
