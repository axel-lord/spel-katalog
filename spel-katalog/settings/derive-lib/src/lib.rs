//! Proc macros for settings.

use ::proc_macro2::TokenStream;
use ::syn::parse::{ParseStream, Parser};

mod derive_static_trait;
mod util;
mod variants;

/// Derive the Help trait.
pub fn derive_help(tokens: TokenStream) -> TokenStream {
    (|input: ParseStream| derive_static_trait::derive_static_trait(input, "Help", "help", "help"))
        .parse2(tokens)
        .unwrap_or_else(::syn::Error::into_compile_error)
}

/// Derive the Title trait.
pub fn derive_title(tokens: TokenStream) -> TokenStream {
    (|input: ParseStream| {
        derive_static_trait::derive_static_trait(input, "Title", "title", "title")
    })
    .parse2(tokens)
    .unwrap_or_else(::syn::Error::into_compile_error)
}

/// Derive the DefaultStr trait.
pub fn derive_default_str(tokens: TokenStream) -> TokenStream {
    (|input: ParseStream| {
        derive_static_trait::derive_static_trait(input, "DefaultStr", "default_str", "default_str")
    })
    .parse2(tokens)
    .unwrap_or_else(::syn::Error::into_compile_error)
}

/// Derive the Variants trait.
pub fn derive_variants(tokens: TokenStream) -> TokenStream {
    variants::parse_variants
        .parse2(tokens)
        .unwrap_or_else(::syn::Error::into_compile_error)
}
