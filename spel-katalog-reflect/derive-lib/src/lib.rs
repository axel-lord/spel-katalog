//! Proc macro implementations.

use ::core::ops::ControlFlow;
use ::std::borrow::Cow;

use ::proc_macro2::TokenStream;
use ::quote::quote;
use ::syn::{
    Attribute, Fields, Ident, Token,
    meta::ParseNestedMeta,
    parenthesized,
    parse::{Parse, ParseStream, Parser},
    parse_quote, parse_quote_spanned,
};

/// Parse an item from tokens and ensure it is a [::syn::ItemEnum].
fn narrow_item_enum(
    tokens: TokenStream,
    name: &str,
    with: impl FnOnce(::syn::ItemEnum) -> ::syn::Result<TokenStream>,
) -> TokenStream {
    match ::syn::Item::parse.parse2(tokens) {
        Err(err) => err.into_compile_error(),
        Ok(::syn::Item::Enum(item)) => with(item).unwrap_or_else(::syn::Error::into_compile_error),
        Ok(item) => {
            ::syn::Error::new_spanned(item, format!("{name} may only be derived for enums"))
                .into_compile_error()
        }
    }
}

/// Implement `Variants` for an enum.
pub fn derive_variants(tokens: TokenStream) -> TokenStream {
    narrow_item_enum(tokens, "Variants", variants)
}

/// Implement `Cycle` for an enum.
pub fn derive_cycle(tokens: TokenStream) -> TokenStream {
    narrow_item_enum(tokens, "Cycle", cycle)
}

/// Implement `AsStr` for an enum.
pub fn derive_as_str(tokens: TokenStream) -> TokenStream {
    narrow_item_enum(tokens, "AsStr", as_str)
}

/// Implement `AsStr` for an enum.
fn as_str(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let crate_path = get_crate_path(&item.attrs, "as_str")?;
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
    let str_rep = item
        .variants
        .iter()
        .map(get_variant_as_str)
        .collect::<Result<Vec<_>, _>>()?;

    let ident = &item.ident;

    Ok(quote! {
        const _: () = {

        impl #crate_path::AsStr for #ident {
            fn as_str<'__a>(&self) -> &'__a str {
                match self {#(
                    Self::#variant_pat => #str_rep,
                )*}
            }
        }

        };
    })
}

/// Implement `Cycle` for an enum.
///
/// # Errors
/// If the enum contains non-unit variants.
fn cycle(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let crate_path = get_crate_path(&item.attrs, "cycle")?;
    let variants = get_unit_variants(&item)?;

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
    let variants = get_unit_variants(&item)?;

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

/// Get crate_path attribute.
fn get_crate_path(attrs: &[Attribute], attr_name: &str) -> ::syn::Result<::syn::ExprPath> {
    get_crate_path_and(attrs, attr_name, |_| Ok(ControlFlow::Continue(())))
}

/// Get top level attributes. returning crate_path attributes and allowing a closure to be ran on
/// other attributes.
fn get_crate_path_and(
    attrs: &[Attribute],
    attr_name: &str,
    mut with: impl FnMut(&ParseNestedMeta) -> ::syn::Result<ControlFlow<()>>,
) -> ::syn::Result<::syn::ExprPath> {
    let mut crate_path = None;
    for attr in attrs {
        let Some(ident) = attr.path().get_ident() else {
            continue;
        };

        if ident != attr_name && ident != "reflect" {
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
                match with(&meta)? {
                    ControlFlow::Continue(_) => {}
                    ControlFlow::Break(_) => return Ok(()),
                };
                if ident != "reflect" {
                    Err(meta.error("unsupported property"))
                } else {
                    Ok(())
                }
            }
        })?;
    }
    let crate_path = crate_path.unwrap_or_else(|| parse_quote!(::spel_katalog_reflect));
    Ok(crate_path)
}

/// Get variant as a string literal, using as_str attribute if avaialable.
fn get_variant_as_str(variant: &::syn::Variant) -> ::syn::Result<Cow<'_, ::syn::LitStr>> {
    let mut str_rep = None;
    for attr in &variant.attrs {
        let Some(ident) = attr.path().get_ident() else {
            continue;
        };

        if ident != "as_str" {
            continue;
        }

        let value = match &attr.meta {
            ::syn::Meta::Path(path) => {
                return Err(::syn::Error::new_spanned(
                    path,
                    "as_str property must be of the 'as_str = _' or 'as_str(_)' format",
                ));
            }
            ::syn::Meta::List(meta_list) => Cow::Owned(meta_list.parse_args()?),
            ::syn::Meta::NameValue(meta_name_value) => match &meta_name_value.value {
                ::syn::Expr::Lit(::syn::ExprLit {
                    lit: ::syn::Lit::Str(lit_str),
                    ..
                }) => Cow::Borrowed(lit_str),
                other => {
                    return Err(::syn::Error::new_spanned(
                        other,
                        "as_str propery must have a string literal value",
                    ));
                }
            },
        };
        str_rep = Some(value);
    }

    Ok(if let Some(str_rep) = str_rep {
        str_rep
    } else {
        let str_rep = variant.ident.to_string();
        Cow::Owned(parse_quote_spanned!(variant.ident.span()=> #str_rep))
    })
}

/// Get idents of fields of enum if they are all unit fields.
fn get_unit_variants(item: &::syn::ItemEnum) -> ::syn::Result<Vec<&Ident>> {
    item.variants
        .iter()
        .map(|variant| {
            if !matches!(variant.fields, Fields::Unit) {
                Err(::syn::Error::new_spanned(
                    &variant.fields,
                    "only unit variants expected",
                ))
            } else {
                Ok(&variant.ident)
            }
        })
        .collect()
}
