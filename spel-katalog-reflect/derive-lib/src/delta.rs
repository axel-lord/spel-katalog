//! Implementation for `Delta` derive macro.

use ::proc_macro2::TokenStream;
use ::quote::quote;

use crate::get::{self, attrl, match_parsed_attr};

/// Implement `Proxy` for a struct.
pub fn delta(item: ::syn::ItemStruct) -> ::syn::Result<::proc_macro2::TokenStream> {
    let mut all_option = false;
    let mut all_skip = false;
    let crate_path = get::crate_path_and(&item.attrs, attrl![delta], |meta| {
        Ok(match_parsed_attr! {
            meta;
            skip => :all_skip,
            option => :all_option,
        })
    })?;

    let generated = item
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let mut skip = all_skip;
            let mut option = all_option;

            get::attrs(&field.attrs, attrl![delta], |meta| {
                Ok(match_parsed_attr! {
                    meta;
                    skip => :skip,
                    option => :option,
                })
            })?;

            let quote = quote! {};
            Ok(((), quote))
        })
        .collect::<::syn::Result<(Vec<_>, TokenStream)>>();

    Ok(quote! {})
}
