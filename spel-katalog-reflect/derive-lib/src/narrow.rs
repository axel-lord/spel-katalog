//! Narrow macro input to specific items.

use ::proc_macro2::TokenStream;
use ::syn::parse::{Parse, Parser};

use crate::soft_err::with_soft_err_stack;

/// Parse an item from tokens and ensure it is a [::syn::ItemEnum].
pub fn narrow_item_enum(
    tokens: TokenStream,
    name: &str,
    with: impl FnOnce(::syn::ItemEnum) -> ::syn::Result<TokenStream>,
) -> TokenStream {
    match ::syn::Item::parse.parse2(tokens) {
        Err(err) => err.into_compile_error(),
        Ok(::syn::Item::Enum(item)) => {
            with_soft_err_stack(|| with(item).unwrap_or_else(::syn::Error::into_compile_error))
        }
        Ok(item) => {
            ::syn::Error::new_spanned(item, format!("{name} may only be derived for enums"))
                .into_compile_error()
        }
    }
}

/// Parse an item from tokens and ensure it is a [::syn::ItemStruct].
pub fn narrow_item_struct(
    tokens: TokenStream,
    name: &str,
    with: impl FnOnce(::syn::ItemStruct) -> ::syn::Result<TokenStream>,
) -> TokenStream {
    match ::syn::Item::parse.parse2(tokens) {
        Err(err) => err.into_compile_error(),
        Ok(::syn::Item::Struct(item)) => {
            with_soft_err_stack(|| with(item).unwrap_or_else(::syn::Error::into_compile_error))
        }
        Ok(item) => {
            ::syn::Error::new_spanned(item, format!("{name} may only be derived for structs"))
                .into_compile_error()
        }
    }
}
