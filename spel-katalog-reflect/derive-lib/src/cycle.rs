//! Implementation for `Cycle` derive macro.

use ::proc_macro2::TokenStream;
use ::quote::quote;

use crate::get;

/// Implement `Cycle` for an enum.
///
/// # Errors
/// If the enum contains non-unit variants.
pub fn cycle(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let crate_path = get::crate_path(&item.attrs, &["cycle"])?;
    let variants = get::unit_variants(&item)?;

    let cycle_next = variants.iter().cycle().skip(1);
    let cycle_prev = variants.iter().cycle().skip(variants.len() - 1);
    let ident = &item.ident;

    Ok(quote! {
        const _: () = {

        #[automatically_derived]
        unsafe impl #crate_path::Cycle for #ident {
            fn cycle_next(&self) -> Self {
                match self {#(
                    Self::#variants => Self::#cycle_next,
                )*}
            }

            fn cycle_prev(&self) -> Self {
                match self {#(
                    Self::#variants => Self::#cycle_prev,
                )*}
            }
        }

        };
    })
}
