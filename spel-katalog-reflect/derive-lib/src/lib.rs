//! Proc macro implementations.

use ::proc_macro2::TokenStream;
use ::quote::quote;
use ::syn::{Attribute, Fields, Ident, Token, parenthesized, parse::ParseStream, parse_quote};

/// Implement `Variants` for an enum.
pub fn derive_variants(tokens: TokenStream) -> TokenStream {
    match ::syn::parse2::<::syn::Item>(tokens) {
        Err(err) => err.into_compile_error(),
        Ok(::syn::Item::Enum(item)) => {
            variants(item).unwrap_or_else(::syn::Error::into_compile_error)
        }
        Ok(item) => ::syn::Error::new_spanned(item, "Variants may only be derived for enums")
            .into_compile_error(),
    }
}

/// Implement `Cycle` for an enum.
pub fn derive_cycle(tokens: TokenStream) -> TokenStream {
    match ::syn::parse2::<::syn::Item>(tokens) {
        Err(err) => err.into_compile_error(),
        Ok(::syn::Item::Enum(item)) => cycle(item).unwrap_or_else(::syn::Error::into_compile_error),
        Ok(item) => ::syn::Error::new_spanned(item, "Cycle may only be derived for enums")
            .into_compile_error(),
    }
}

/// Get crate_path attribute.
fn get_crate_path(attrs: &[Attribute], attr_name: &str) -> ::syn::Result<::syn::ExprPath> {
    let mut crate_path = None;
    for attr in attrs {
        let Some(ident) = attr.path().get_ident() else {
            continue;
        };

        if ident != attr_name {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            let mut parse_crate_path = |tokens: ParseStream| -> Result<(), ::syn::Error> {
                match tokens.parse::<::syn::Expr>()? {
                    ::syn::Expr::Path(path) => {
                        crate_path = Some(path);
                        Ok(())
                    }
                    _ => Err(meta.error("crate_path must be a module path")),
                }
            };
            if meta.path.is_ident("crate_path") {
                if meta.input.peek(Token![=]) {
                    let tokens = meta.value()?;
                    parse_crate_path(tokens)
                } else {
                    let content;
                    parenthesized!(content in meta.input);
                    parse_crate_path(&content)
                }
            } else {
                Err(meta.error("unsupported property"))
            }
        })?;
    }
    let crate_path = crate_path.unwrap_or_else(|| parse_quote!(::spel_katalog_reflect));
    Ok(crate_path)
}

/// Get idents of fields of enum if they are all unit fields.
fn get_unit_fields(item: &::syn::ItemEnum) -> ::syn::Result<Vec<&Ident>> {
    let mut variants = Vec::new();
    for variant in &item.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(::syn::Error::new_spanned(
                &variant.fields,
                "only unit variants expected",
            ));
        }
        variants.push(&variant.ident);
    }
    Ok(variants)
}

/// Implement `Cycle` for an enum.
///
/// # Errors
/// If the enum contains non-unit variants.
fn cycle(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let crate_path = get_crate_path(&item.attrs, "cycle")?;
    let variants = get_unit_fields(&item)?;

    let cycle_next = variants.iter().cycle().skip(1);
    let cycle_prev = variants.iter().cycle().skip(variants.len() - 1);
    let ident = &item.ident;

    Ok(quote! {
        const _: () = {

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

/// Implement `Variants` for an enum.
///
/// # Errors
/// If the enum contains non-unit variants.
fn variants(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let crate_path = get_crate_path(&item.attrs, "variants")?;
    let variants = get_unit_fields(&item)?;

    let indices = 0..variants.len();
    let ident = &item.ident;

    Ok(quote! {
        const _: () = {

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
