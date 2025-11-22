//! Proc macro implementations.

use ::proc_macro2::TokenStream;

use crate::narrow::{narrow_item_enum, narrow_item_struct};

/// Implement `Variants` for an enum.
pub fn derive_variants(tokens: TokenStream) -> TokenStream {
    narrow_item_enum(tokens, "Variants", variants::variants)
}

/// Implement `Cycle` for an enum.
pub fn derive_cycle(tokens: TokenStream) -> TokenStream {
    narrow_item_enum(tokens, "Cycle", cycle::cycle)
}

/// Implement `AsStr` for an enum.
pub fn derive_as_str(tokens: TokenStream) -> TokenStream {
    narrow_item_enum(tokens, "AsStr", as_str::as_str)
}

/// Implement `FromStr` for an enum.
pub fn derive_from_str(tokens: TokenStream) -> TokenStream {
    narrow_item_enum(tokens, "FromStr", from_str::from_str)
}

/// Implement `OptionDefault` for an enum.
pub fn derive_proxy(tokens: TokenStream) -> TokenStream {
    narrow_item_struct(tokens, "Proxy", proxy::proxy)
}

mod as_str;
mod cycle;
mod delta;
mod from_str;
mod get;
mod narrow;
mod proxy;
mod soft_err;
mod variants;
