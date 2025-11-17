//! Derive macros used for reflection.

use ::proc_macro::TokenStream;

/// Derive implementation of `Variants` for an enum.
#[proc_macro_derive(Variants, attributes(variants))]
pub fn derive_variants(item: TokenStream) -> TokenStream {
    ::spel_katalog_reflect_derive_lib::derive_variants(item.into()).into()
}

/// Derive implementation of `Cycle` for an enum.
#[proc_macro_derive(Cycle, attributes(cycle))]
pub fn derive_cycle(item: TokenStream) -> TokenStream {
    ::spel_katalog_reflect_derive_lib::derive_cycle(item.into()).into()
}
