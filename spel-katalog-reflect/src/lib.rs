//! Reflection utilities.

/// Trait for simple enums to provide all values.
///
/// # Safety
/// The `VARIANTS` associated constant must contain all variants.
/// `index_of` must return the correct index.
/// `cycle_next` must return the cyclic next variant in `VARIANTS`.
/// `cycle_prev` must return the cyclic previous variant in `VARIANTS`.
pub unsafe trait Variants
where
    Self: 'static + Sized,
{
    /// All values for enum.
    const VARIANTS: &[Self];

    /// Get index of variant in variant array.
    fn index_of(&self) -> usize;

    /// Get the next variant. For the last variant will return the first variant.
    fn cycle_next(&self) -> Self;

    /// Get the next variant. For the last variant will return the first variant.
    fn cycle_prev(&self) -> Self;
}

#[doc(inline)]
pub use ::spel_katalog_reflect_derive::Variants;

#[cfg(test)]
mod tests {
    use super::*;
    use ::pretty_assertions::assert_eq;

    #[test]
    fn derived() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Variants)]
        #[variants(crate_path = crate)]
        enum VariantsTestEnum {
            First,
            Second,
            Third,
            Fourth,
        }
        use VariantsTestEnum::*;

        assert_eq!(First.index_of(), 0);
        assert_eq!(First.cycle_next(), Second);
        assert_eq!(First.cycle_prev(), Fourth);

        assert_eq!(Fourth.index_of(), 3);
        assert_eq!(Fourth.cycle_next(), First);
        assert_eq!(Fourth.cycle_prev(), Third);

        assert_eq!(VariantsTestEnum::VARIANTS, &[First, Second, Third, Fourth]);
    }
}
