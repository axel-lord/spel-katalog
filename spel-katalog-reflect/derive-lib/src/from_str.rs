//! Implementation for `FromStr` derive macro.

use ::std::borrow::Cow;

use ::convert_case::Casing;
use ::proc_macro2::TokenStream;
use ::quote::quote;
use ::rustc_hash::FxHashMap;
use ::syn::LitStr;

use crate::get::{self, match_parsed_attr};

/// Implement `FromStr` for an enum.
pub fn from_str(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let mut impl_try_from = false;
    let mut case_convert = false;
    let crate_path = get::crate_path_and(&item.attrs, &["from_str"], |meta| {
        Ok(match_parsed_attr! {
            meta;
            try_from => impl_try_from = true,
            case_convert => :case_convert,
        })
    })?;

    let variants = get::unit_variants(&item)?;
    let str_rep = get::variants_as_str_reprs(&item)?;

    let arms = if !case_convert {
        str_rep
            .into_iter()
            .zip(variants.iter().copied())
            .collect::<Vec<_>>()
    } else {
        let spanned_strings = str_rep
            .iter()
            .map(|lit_str| (lit_str.span(), lit_str.value()))
            .collect::<Vec<_>>();

        let arms = ::convert_case::Case::all_cases()
            .iter()
            .filter(|case| {
                matches!(
                    case,
                    ::convert_case::Case::Snake
                        | ::convert_case::Case::Constant
                        | ::convert_case::Case::UpperSnake
                        | ::convert_case::Case::Ada
                        | ::convert_case::Case::Kebab
                        | ::convert_case::Case::Cobol
                        | ::convert_case::Case::UpperKebab
                        | ::convert_case::Case::Train
                        | ::convert_case::Case::Flat
                        | ::convert_case::Case::UpperFlat
                        | ::convert_case::Case::Pascal
                        | ::convert_case::Case::UpperCamel
                        | ::convert_case::Case::Camel
                        | ::convert_case::Case::Lower
                        | ::convert_case::Case::Upper
                        | ::convert_case::Case::Title
                        | ::convert_case::Case::Sentence
                )
            })
            .flat_map(|case| {
                spanned_strings
                    .iter()
                    .zip(variants.iter().copied())
                    .map(|((span, s), variant)| {
                        let str_rep = LitStr::new(&s.to_case(*case), *span);

                        (Cow::Owned(str_rep), variant)
                    })
            })
            .collect::<FxHashMap<_, _>>();

        Vec::from_iter(arms)
    };

    let arms = arms
        .iter()
        .map(|(str_rep, variant)| quote! { #str_rep => Ok(Self::#variant) });

    let ident = &item.ident;

    let try_from = impl_try_from.then(|| {
        quote! {
            #[automatically_derived]
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

        #[automatically_derived]
        impl #crate_path::FromStr for #ident {
            type Err = #crate_path::UnknownVariant;

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                match s {
                    #( #arms, )*
                    _ => Err(#crate_path::UnknownVariant),
                }
            }
        }

        #try_from

        };
    })
}
