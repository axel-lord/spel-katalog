use ::proc_macro2::TokenStream;
use ::quote::{ToTokens, quote};
use ::syn::{
    Expr, Field, Ident, ItemEnum, Pat, Token, Variant,
    parse::{ParseStream, Parser},
    punctuated::Punctuated,
};

use crate::util::parse_settings_attr;

fn shifted<'a, T: 'a>(iter: impl 'a + IntoIterator<Item = T>) -> impl 'a + IntoIterator<Item = T> {
    let mut iter = iter.into_iter();
    let first = iter.next();
    iter.chain(first)
}

fn variant_match_pattern(ident: &Ident, variant: &Variant) -> ::syn::Result<Pat> {
    let variant_ident = &variant.ident;
    Pat::parse_single.parse2(match &variant.fields {
        ::syn::Fields::Named(_) => quote! {#ident::#variant_ident{..}},
        ::syn::Fields::Unnamed(_) => quote! {#ident::#variant_ident(..)},
        ::syn::Fields::Unit => quote! {#ident::#variant_ident},
    })
}

fn field_exprs(fields: &Punctuated<Field, Token![,]>) -> ::syn::Result<Vec<TokenStream>> {
    fields
        .iter()
        .map(|field| {
            let mut force_expr = None;
            parse_settings_attr(&field.attrs, "variants", |meta| {
                let list = meta.require_list()?;
                list.parse_nested_meta(|meta| {
                    if meta.path.is_ident("expr") {
                        let value = meta.value()?;
                        force_expr = Some(value.parse::<Expr>()?);
                        Ok(())
                    } else {
                        Err(meta.error("unexpected attribute, expected 'expr'"))
                    }
                })
            })?;

            let get_assign = |expr: &Expr| {
                if let Some(field_ident) = &field.ident {
                    quote! { #field_ident: #expr }
                } else {
                    expr.to_token_stream()
                }
            };

            if let Some(expr) = force_expr {
                return Ok(get_assign(&expr));
            };

            todo!()
        })
        .collect()
}

fn variant_expr(ident: &Ident, variant: &Variant) -> ::syn::Result<TokenStream> {
    let variant_ident = &variant.ident;
    Ok(match &variant.fields {
        ::syn::Fields::Named(named) => {
            if named.named.is_empty() {
                quote! {#ident::#variant_ident{}}
            } else {
                let exprs = field_exprs(&named.named)?;
                quote! {#ident::#variant_ident { #(#exprs),* }}
            }
        }
        ::syn::Fields::Unnamed(unnamed) => {
            if unnamed.unnamed.is_empty() {
                quote! {#ident::#variant_ident()}
            } else {
                let exprs = field_exprs(&unnamed.unnamed)?;
                quote! {#ident::#variant_ident ( #(#exprs),* )}
            }
        }
        ::syn::Fields::Unit => quote! {#ident::#variant_ident},
    })
}

/// Parse portion of variants impl.
pub(crate) fn parse_variants(input: ParseStream) -> ::syn::Result<TokenStream> {
    let item_enum = input.parse::<ItemEnum>()?;
    let ident = &item_enum.ident;

    let (variants, arms) = item_enum
        .variants
        .iter()
        .zip(shifted(&item_enum.variants))
        .map(|(variant, next_variant)| {
            let match_pattern = variant_match_pattern(ident, variant)?;

            let expr = variant_expr(ident, variant)?;
            let next_expr = variant_expr(ident, next_variant)?;

            let arm = quote! {
                #match_pattern => const { #next_expr },
            };

            Ok((expr, arm))
        })
        .collect::<::syn::Result<(Vec<_>, Vec<_>)>>()?;

    Ok(quote! {
        unsafe impl ::spel_katalog_settings_traits::TrustedVariants for #ident {
            const VARIANTS: &[#ident] = &[#(#variants),*];

            #[inline]
            fn cycle(&self) -> Self
            where
                Self: ::core::cmp::PartialEq + ::core::clone::Clone,
            {
                match self {
                    #(#arms)*
                }
            }
        }
    })
}
