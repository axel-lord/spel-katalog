//! Implementation for `FromStr` derive macro.

use ::core::ops::ControlFlow;

use ::proc_macro2::TokenStream;
use ::quote::quote;

use crate::get;

/// Implement `FromStr` for an enum.
pub fn from_str(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let mut impl_try_from = false;
    let crate_path = get::crate_path_and(&item.attrs, &["from_str"], |meta| {
        Ok(if meta.path.is_ident("try_from") {
            impl_try_from = true;
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        })
    })?;

    let variants = get::unit_variants(&item)?;
    let str_rep = get::variants_as_str_reprs(&item)?;
    let ident = &item.ident;

    let try_from = impl_try_from.then(|| {
        quote! {
            impl ::core::convert::TryFrom<&str> for #ident
            {
                type Error = #crate_path::UnknownVariant;

                fn try_from(value: &str) -> ::core::result::Result<Self, Self::Error> {
                    <Self as #crate_path::FromStr>::from_str(value)
                }
            }

        }
    });

    Ok(quote! {
        const _:() = {

        impl #crate_path::FromStr for #ident {
            type Err = #crate_path::UnknownVariant;

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                match s {
                    #( #str_rep => Ok(Self::#variants), )*
                    _ => Err(#crate_path::UnknownVariant),
                }
            }
        }

        #try_from

        };
    })
}
