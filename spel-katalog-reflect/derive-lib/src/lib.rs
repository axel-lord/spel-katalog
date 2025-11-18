//! Proc macro implementations.

use ::core::ops::ControlFlow;
use ::std::{borrow::Cow, cell::RefCell, mem};

use ::proc_macro2::TokenStream;
use ::quote::quote;
use ::syn::{
    Attribute, Fields, Ident, Token,
    meta::ParseNestedMeta,
    parenthesized,
    parse::{Parse, ParseStream, Parser},
    parse_quote, parse_quote_spanned,
};

thread_local! {
    static SOFT_ERR_STACK: RefCell<Vec<::syn::Error>> = RefCell::new(Vec::new());
}

/// Push an error to soft error stack.
fn push_soft_err(err: ::syn::Error) {
    SOFT_ERR_STACK.with(|stack| stack.borrow_mut().push(err));
}

fn with_soft_err_stack(f: impl FnOnce() -> TokenStream) -> TokenStream {
    let mut backup = Vec::new();
    SOFT_ERR_STACK.with(|stack| mem::swap(&mut backup, &mut *stack.borrow_mut()));
    let mut tokens = f();
    SOFT_ERR_STACK.with(|stack| mem::swap(&mut backup, &mut *stack.borrow_mut()));

    for err in backup {
        tokens.extend(err.into_compile_error());
    }

    tokens
}

/// Parse an item from tokens and ensure it is a [::syn::ItemEnum].
fn narrow_item_enum(
    tokens: TokenStream,
    name: &str,
    with: impl FnOnce(::syn::ItemEnum) -> ::syn::Result<TokenStream>,
) -> TokenStream {
    match ::syn::Item::parse.parse2(tokens) {
        Err(err) => err.into_compile_error(),
        Ok(::syn::Item::Enum(item)) => {
            with_soft_err_stack(|| with(item).unwrap_or_else(::syn::Error::into_compile_error))
        }
        Ok(item) => {
            ::syn::Error::new_spanned(item, format!("{name} may only be derived for enums"))
                .into_compile_error()
        }
    }
}

/// Parse an item from tokens and ensure it is a [::syn::ItemStruct].
fn narrow_item_struct(
    tokens: TokenStream,
    name: &str,
    with: impl FnOnce(::syn::ItemStruct) -> ::syn::Result<TokenStream>,
) -> TokenStream {
    match ::syn::Item::parse.parse2(tokens) {
        Err(err) => err.into_compile_error(),
        Ok(::syn::Item::Struct(item)) => {
            with_soft_err_stack(|| with(item).unwrap_or_else(::syn::Error::into_compile_error))
        }
        Ok(item) => {
            ::syn::Error::new_spanned(item, format!("{name} may only be derived for structs"))
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

/// Implement `FromStr` for an enum.
pub fn derive_from_str(tokens: TokenStream) -> TokenStream {
    narrow_item_enum(tokens, "FromStr", from_str)
}

/// Implement `OptionDefault` for an enum.
pub fn derive_option_default(tokens: TokenStream) -> TokenStream {
    narrow_item_struct(tokens, "OptionDefault", option_default)
}

/// Implement `OptDefault` for a struct.
fn option_default(item: ::syn::ItemStruct) -> ::syn::Result<TokenStream> {
    let mut all_option = false;
    let _crate_path = get_crate_path_and(&item.attrs, "option_default", |meta| {
        Ok(if meta.path.is_ident("option") {
            all_option = true;
            ControlFlow::Break(())
        } else if meta.path.is_ident("no_option") {
            all_option = false;
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        })
    })?;

    let getters = item
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let mut is_option = all_option;
            get_attrs(&field.attrs, "option_default", |meta| {
                Ok(if meta.path.is_ident("option") {
                    is_option = true;
                    ControlFlow::Break(())
                } else if meta.path.is_ident("no_option") {
                    is_option = false;
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                })
            })?;

            let ty = unwrap_ty(&field.ty);
            let doc = field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("doc"));

            if is_option {

            let ::syn::Type::Path(ty_path) = ty else {
                push_soft_err(::syn::Error::new_spanned(ty, "expected a type path"));
                return Ok(TokenStream::default());
            };

            let Some(::syn::PathArguments::AngleBracketed(arguments)) = ty_path
                .path
                .segments
                .last()
                .map(|segment| &segment.arguments)
            else {
                push_soft_err(::syn::Error::new_spanned(
                    ty_path,
                    format!("expected a type argument"),
                ));
                return Ok(TokenStream::default());
            };

            let Some(::syn::GenericArgument::Type(ty)) = arguments
                .args
                .iter()
                .find(|arg| matches!(arg, ::syn::GenericArgument::Type(..)))
            else {
                push_soft_err(::syn::Error::new_spanned(
                    arguments,
                    "expected at least one type argument",
                ));
                return Ok(TokenStream::default());
            };

            if let Some(ident) = &field.ident {
                Ok(quote! {
                    #(#doc)*
                    pub fn #ident(self) -> &'__this #ty {
                        static DEFAULT: ::std::sync::OnceLock<#ty> = ::std::sync::OnceLock::new();
                        if let Some(value) = &self.0.#ident {
                            value
                        } else {
                            DEFAULT.get_or_init(|| ::core::default::Default::default())
                        }
                    }
                })
            } else {
                let ident = ::quote::format_ident!("_{i}");
                Ok(quote! {
                    #(#doc)*
                    pub fn #ident(self) -> &'__this #ty {
                        static DEFAULT: ::std::sync::OnceLock<#ty> = ::std::sync::OnceLock::new();
                        if let Some(value) = &self.0.#i {
                            value
                        } else {
                            DEFAULT.get_or_init(|| ::core::default::Default::default())
                        }
                    }
                })
            }
            } else {
            if let Some(ident) = &field.ident {
                Ok(quote! {
                    #(#doc)*
                    pub fn #ident(self) -> &'__this #ty {
                        &self.0.#ident
                    }
                })
            } else {
                let ident = ::quote::format_ident!("_{i}");
                Ok(quote! {
                    #(#doc)*
                    pub fn #ident(self) -> &'__this #ty {
                        &self.0.#i
                    }
                })
            }
            }
        })
        .collect::<::syn::Result<TokenStream>>()?;

    let ident = &item.ident;

    Ok(quote! {
        const _: () = {
            #[derive(Clone, Copy)]
            #[doc = "Proxy Object"]
            struct __Proxy<'__this>(&'__this #ident);
            impl<'__this> __Proxy<'__this> {
                #getters
            }
        };
    })
}

/// Implement `FromStr` for an enum.
fn from_str(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let mut impl_try_from = false;
    let crate_path = get_crate_path_and(&item.attrs, "from_str", |meta| {
        Ok(if meta.path.is_ident("try_from") {
            impl_try_from = true;
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        })
    })?;

    let variants = get_unit_variants(&item)?;
    let str_rep = get_variants_as_str_reprs(&item)?;
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

/// Implement `AsStr` for an enum.
fn as_str(item: ::syn::ItemEnum) -> ::syn::Result<TokenStream> {
    let mut impl_display = false;
    let mut impl_as_ref = false;

    let crate_path = get_crate_path_and(&item.attrs, "as_str", |meta| {
        Ok(if meta.path.is_ident("display") {
            impl_display = true;
            ControlFlow::Break(())
        } else if meta.path.is_ident("as_ref") {
            impl_as_ref = true;
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
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
    let str_rep = get_variants_as_str_reprs(&item)?;

    let ident = &item.ident;

    let as_ref = impl_as_ref.then(|| {
        quote! {
            impl ::core::convert::AsRef<str> for #ident {
                fn as_ref(&self) -> &str {
                    <Self as #crate_path::AsStr>::as_str(self)
                }
            }
        }
    });

    let display = impl_display.then(|| {
        quote! {
            impl ::core::fmt::Display for #ident {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.write_str(<Self as #crate_path::AsStr>::as_str(self))
                }
            }
        }
    });

    Ok(quote! {
        const _: () = {

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

fn get_attrs(
    attrs: &[Attribute],
    attr_name: &str,
    mut with: impl FnMut(&ParseNestedMeta) -> ::syn::Result<ControlFlow<()>>,
) -> ::syn::Result<()> {
    let mut attr_parsed = false;
    for attr in attrs {
        if attr.path().is_ident("reflect") {
            attr.parse_nested_meta(|meta| {
                _ = with(&meta)?;
                Ok(())
            })?;
            if attr_parsed {
                push_soft_err(::syn::Error::new_spanned(
                    attr.path(),
                    format!("reflect attribute should not be placed after {attr_name} attribute"),
                ));
            }
        } else if attr.path().is_ident(attr_name) {
            attr.parse_nested_meta(|meta| {
                if let ControlFlow::Continue(_) = with(&meta)? {
                    push_soft_err(meta.error("unsupported property"));
                }
                Ok(())
            })?;
            attr_parsed = true;
        }
    }
    Ok(())
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
    get_attrs(attrs, attr_name, |meta| {
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
                parse_crate_path(tokens)?;
            } else {
                let content;
                parenthesized!(content in meta.input);
                parse_crate_path(&content)?;
            }
            Ok(ControlFlow::Break(()))
        } else {
            with(meta)
        }
    })?;
    Ok(crate_path.unwrap_or_else(|| parse_quote!(::spel_katalog_reflect)))
}

/// Get variants as string literals, using as_str attribute if avaialable.
fn get_variants_as_str_reprs(item: &::syn::ItemEnum) -> ::syn::Result<Vec<Cow<'_, ::syn::LitStr>>> {
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
    item.variants.iter().map(get_variant_as_str).collect()
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

/// Unwrap a syn type.
fn unwrap_ty(ty: &::syn::Type) -> &::syn::Type {
    let mut ty = ty;
    loop {
        match ty {
            ::syn::Type::Group(::syn::TypeGroup { elem, .. })
            | ::syn::Type::Paren(::syn::TypeParen { elem, .. }) => {
                ty = &elem;
            }
            ty => return ty,
        }
    }
}
