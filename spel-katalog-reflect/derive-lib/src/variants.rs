//! Implementation for `AsStr` derive macro.

use ::proc_macro2::TokenStream;
use ::quote::quote;

use crate::get;

/// Implement `Variants` for an enum.
///
/// # Errors
/// If the enum contains non-unit variants.
pub fn variants(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let crate_path = get::crate_path(&item.attrs, &["variants"])?;
    let variants = get::unit_variants(&item)?;

    let indices = 0..variants.len();
    let ident = &item.ident;

    Ok(quote! {
        const _: () = {

        #[automatically_derived]
        unsafe impl #crate_path::Variants for #ident {
            const VARIANTS: &[Self] = &[#(Self::#variants),*];

            fn index_of(&self) -> usize {
                match self {#(
                    Self::#variants => #indices,
                )*}
            }
        }

        };
    })
}
