//! Proc macros for settings.

use ::proc_macro2::{Span, TokenStream};
use ::quote::quote;
use ::syn::{
    Arm, Attribute, Expr, Ident, ItemEnum, Meta, MetaList, MetaNameValue, Pat, Token, Variant,
    Visibility,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
};

/// Parse any type class (struct, enum, union) and throw away the result
fn parse_any_type_class(input: ParseStream) -> ::syn::Result<()> {
    let lookahead = input.lookahead1();
    if lookahead.peek(Token![struct]) {
        Ok(_ = input.parse::<Token![struct]>())
    } else if lookahead.peek(Token![enum]) {
        Ok(_ = input.parse::<Token![enum]>())
    } else if lookahead.peek(Token![union]) {
        Ok(_ = input.parse::<Token![union]>())
    } else {
        Err(lookahead.error())
    }
}

/// Derive the Help trait.
pub fn derive_help(tokens: TokenStream) -> TokenStream {
    (|input: ParseStream| derive_static_trait(input, "Help", "help", "help"))
        .parse2(tokens)
        .unwrap_or_else(::syn::Error::into_compile_error)
}

/// Derive the Title trait.
pub fn derive_title(tokens: TokenStream) -> TokenStream {
    (|input: ParseStream| derive_static_trait(input, "Title", "title", "title"))
        .parse2(tokens)
        .unwrap_or_else(::syn::Error::into_compile_error)
}

/// Derive the DefaultStr trait.
pub fn derive_default_str(tokens: TokenStream) -> TokenStream {
    (|input: ParseStream| derive_static_trait(input, "DefaultStr", "default_str", "default_str"))
        .parse2(tokens)
        .unwrap_or_else(::syn::Error::into_compile_error)
}

/// Derive a trait with a single method returning a static value.
fn derive_static_trait(
    input: ParseStream,
    trait_name: &str,
    attr_name: &str,
    method_name: &str,
) -> ::syn::Result<TokenStream> {
    let attrs = input.call(Attribute::parse_outer)?;
    input.parse::<Visibility>()?;
    input.call(parse_any_type_class)?;
    let ident = input.parse::<Ident>()?;
    input.parse::<TokenStream>()?;

    let mut expr_attr = None;
    for attr in attrs {
        if attr.path().is_ident(attr_name) {
            attr.meta.require_name_value()?;
            if let Meta::NameValue(MetaNameValue { value, .. }) = attr.meta {
                expr_attr = Some(value);
            }
        } else if attr.path().is_ident("settings") {
            attr.meta.require_list()?;
            if let Meta::List(list) = attr.meta {
                let items = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens)?;
                for meta in items {
                    if meta.path().is_ident(attr_name) {
                        meta.require_name_value()?;
                        if let Meta::NameValue(MetaNameValue { value, .. }) = meta {
                            expr_attr = Some(value);
                        }
                    }
                }
            }
        }
    }

    let Some(expr_attr) = expr_attr else {
        return Err(input.error(format!(
            "'{attr_name}' attribute should be present when deriving {trait_name}"
        )));
    };

    let trait_ident = Ident::new(trait_name, Span::call_site());
    let method_ident = Ident::new(method_name, Span::call_site());

    Ok(quote! {
        impl ::spel_katalog_settings_traits::#trait_ident for #ident {
            fn #method_ident() -> &'static str {
                #expr_attr
            }
        }

    })
}

/// Derive the Variants trait.
pub fn derive_variants(tokens: TokenStream) -> TokenStream {
    parse_variants
        .parse2(tokens)
        .unwrap_or_else(::syn::Error::into_compile_error)
}

/// Parse portion of variants impl.
fn parse_variants(input: ParseStream) -> ::syn::Result<TokenStream> {
    let item_enum = input.parse::<ItemEnum>()?;
    let ident = &item_enum.ident;

    fn shifted<'a, T: 'a>(
        iter: impl 'a + IntoIterator<Item = T>,
    ) -> impl 'a + IntoIterator<Item = T> {
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

    fn parse_meta(meta: &MetaList, force_expr: &mut Option<Expr>) -> ::syn::Result<()> {
        meta.parse_nested_meta(|meta| {
            if meta.path.is_ident("expr") {
                *force_expr = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("unsupported variants attribute"))
            }
        })
    }

    fn variant_expr(ident: &Ident, variant: &Variant) -> ::syn::Result<Expr> {
        let variant_ident = &variant.ident;
        let mut force_expr = None;
        for attr in &variant.attrs {
            if attr.path().is_ident("variants") {
                parse_meta(attr.meta.require_list()?, &mut force_expr)?;
            } else if attr.path().is_ident("settings") {
                let list = attr.meta.require_list()?;
                Punctuated::<Meta, Token![,]>::parse_terminated
                    .parse2(list.tokens.clone())?
                    .into_iter()
                    .try_for_each(|meta| -> ::syn::Result<_> {
                        if meta.path().is_ident("variants") {
                            parse_meta(meta.require_list()?, &mut force_expr)?;
                        }
                        Ok(())
                    })?;
            }
        }
        if let Some(force_expr) = force_expr {
            return Ok(force_expr);
        };
        Expr::parse.parse2(match &variant.fields {
            ::syn::Fields::Named(_) => quote! {#ident::#variant_ident{}},
            ::syn::Fields::Unnamed(_) => quote! {#ident::#variant_ident()},
            ::syn::Fields::Unit => quote! {#ident::#variant_ident},
        })
    }

    let (variants, arms) = item_enum
        .variants
        .iter()
        .zip(shifted(&item_enum.variants))
        .map(|(variant, next_variant)| {
            let match_pattern = variant_match_pattern(ident, variant)?;

            let expr = variant_expr(ident, variant)?;
            let next_expr = variant_expr(ident, next_variant)?;

            let arm = Arm::parse.parse2(quote! {
                #match_pattern => const { #next_expr },
            })?;

            Ok((expr, arm))
        })
        .collect::<::syn::Result<(Vec<Expr>, Vec<Arm>)>>()?;

    Ok(quote! {
        unsafe impl ::spel_katalog_settings_traits::Variants for #ident {
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
