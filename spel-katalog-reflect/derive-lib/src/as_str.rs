//! Implementation for `AsStr` derive macro.

use ::proc_macro2::TokenStream;
use ::quote::quote;
use ::syn::{Fields, parse_quote};

use crate::get::{self, match_parsed_attr};

/// Implement `AsStr` for an enum.
pub fn as_str(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let mut impl_display = false;
    let mut impl_as_ref = false;

    let crate_path = get::crate_path_and(&item.attrs, &["as_str"], |meta| {
        Ok(match_parsed_attr! {
            meta;
            "display" => impl_display = true,
            "as_ref" => impl_as_ref = true,
        })
    })?;

    let variant_pat = item
        .variants
        .iter()
        .map(|variant| {
            let ident = &variant.ident;

            match variant.fields {
                Fields::Named(..) => parse_quote!(#ident{..}),
                Fields::Unnamed(..) => parse_quote!(#ident(..)),
                Fields::Unit => parse_quote!(#ident),
            }
        })
        .collect::<Vec<::syn::Pat>>();
    let str_rep = get::variants_as_str_reprs(&item)?;

    let ident = &item.ident;

    let as_ref = impl_as_ref.then(|| {
        quote! {
            #[automatically_derived]
            impl ::core::convert::AsRef<str> for #ident {
                fn as_ref(&self) -> &str {
                    <Self as #crate_path::AsStr>::as_str(self)
                }
            }
        }
    });

    let display = impl_display.then(|| {
        quote! {
            #[automatically_derived]
            impl ::core::fmt::Display for #ident {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(<Self as #crate_path::AsStr>::as_str(self))
                }
            }
        }
    });

    Ok(quote! {
        const _: () = {

        #[automatically_derived]
        impl #crate_path::AsStr for #ident {
            fn as_str<'__a>(&self) -> &'__a str {
                match self {#(
                    Self::#variant_pat => #str_rep,
                )*}
            }
        }

        #as_ref
        #display

        };
    })
}
