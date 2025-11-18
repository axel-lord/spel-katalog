//! Derive macros used for reflection.

use ::proc_macro::TokenStream;

/// Derive implementation of `Variants` for an enum.
#[proc_macro_derive(Variants, attributes(variants, reflect))]
pub fn derive_variants(item: TokenStream) -> TokenStream {
    ::spel_katalog_reflect_derive_lib::derive_variants(item.into()).into()
}

/// Derive implementation of `Cycle` for an enum.
#[proc_macro_derive(Cycle, attributes(cycle, reflect))]
pub fn derive_cycle(item: TokenStream) -> TokenStream {
    ::spel_katalog_reflect_derive_lib::derive_cycle(item.into()).into()
}

/// Derive implementation of `AsStr` for an enum.
///
/// With the `as_ref` and `display` attributes `AsRef<str>` and `Display`
/// will also be derived using `AsStr` implementation.
#[proc_macro_derive(AsStr, attributes(as_str, reflect))]
pub fn derive_as_str(item: TokenStream) -> TokenStream {
    ::spel_katalog_reflect_derive_lib::derive_as_str(item.into()).into()
}

/// Derive implementation of `FromStr` for an enum.
///
/// With the `try_from` attribute `TryFrom<&str>`
/// will also be derived using `FromStr` implementation.
#[proc_macro_derive(FromStr, attributes(from_str, reflect))]
pub fn derive_from_str(item: TokenStream) -> TokenStream {
    ::spel_katalog_reflect_derive_lib::derive_from_str(item.into()).into()
}

/// Derive implementation of `OptionDefault` for an enum.
///
/// With the `option` or `no_option` attribute on struct or fields, set either
/// all fields or single field as being/not being an option.
///
/// The `default` attribute in `name = value`, or `list()` form, can be used to set
/// an expression to use instead of `Default::default()`.
///
/// `some_pattern` in `name = value`, or `list()` form, replaces the path part of
/// matching `Some(..)` for option fields. This combined with the way only the first type
/// argument is used for the returned type may be used to allow types other than `Option`
/// for fields, such as `Result`.
#[proc_macro_derive(OptionDefault, attributes(option_default, reflect))]
pub fn derive_option_default(item: TokenStream) -> TokenStream {
    ::spel_katalog_reflect_derive_lib::derive_option_default(item.into()).into()
}
