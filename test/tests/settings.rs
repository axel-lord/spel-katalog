//! Test settings.

use ::core::marker::PhantomData;

use ::pretty_assertions::assert_eq;
use ::spel_katalog_settings_traits::*;

/// An enum value to test,
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    TrustedVariants,
    DefaultStr,
    Help,
    Title,
)]
#[settings(default_str = "A", help = "An enum value to test", title = "EValue")]
enum EnumValue {
    A,
    B(),
    C {},
    D(#[variants(expr = PhantomData)] PhantomData<()>),
    E {
        #[settings(variants(expr = PhantomData))]
        _p: PhantomData<fn() -> usize>,
    },
}

#[test]
fn string_values() {
    assert_eq!(EnumValue::default_str(), "A");
    assert_eq!(EnumValue::help(), "An enum value to test");
    assert_eq!(EnumValue::title(), "EValue");
}

#[test]
fn variants() {
    let v = &[
        EnumValue::A,
        EnumValue::B(),
        EnumValue::C {},
        EnumValue::D(PhantomData),
        EnumValue::E { _p: PhantomData },
    ];
    assert_eq!(v.len(), EnumValue::VARIANTS.len());
    v.iter()
        .zip(EnumValue::VARIANTS)
        .for_each(|(v, variant)| assert_eq!(v, variant));
    v.iter()
        .zip(EnumValue::variants())
        .for_each(|(v, variant)| assert_eq!(v, variant));
    EnumValue::VARIANTS
        .iter()
        .zip(EnumValue::variants())
        .for_each(|(v1, v2)| assert_eq!(v1, v2));
}
#[test]
fn cycle() {
    let next = EnumValue::VARIANTS.iter().cycle().skip(1).copied();
    EnumValue::VARIANTS
        .iter()
        .zip(next)
        .for_each(|(variant, next)| assert_eq!(variant.cycle(), next));
}
