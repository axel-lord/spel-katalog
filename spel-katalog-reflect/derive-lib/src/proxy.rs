//! Implementation for `Proxy` derive macro.

use ::proc_macro2::TokenStream;
use ::quote::{ToTokens, format_ident, quote};
use ::syn::{Ident, parse_quote};

use crate::{
    ext::ResultExt,
    get::{self, match_parsed_attr},
    soft_err::push_soft_err,
};

/// Implement `Proxy` for a struct.
pub fn proxy(item: ::syn::ItemStruct) -> ::syn::Result<TokenStream> {
    let mut all_option = false;
    let mut all_getter = false;
    let mut proxy_debug = false;
    let mut proxy_name = None;
    let mut deref_to_proxy = false;
    let mut as_ref_proxy = false;
    let crate_path = get::crate_path_and(&item.attrs, &["proxy"], |meta| {
        Ok(match_parsed_attr! {
            meta;
            as_ref => :as_ref_proxy,
            debuf => :proxy_debug,
            deref => :deref_to_proxy,
            getter => :all_getter,
            option => :all_option,
            proxy_name => proxy_name = Some(get::list_or_name_value(meta.input, get::ident_from_expr("proxy_name"))?),
        })
    })?;

    let getters = item
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let mut is_option = all_option;
            let mut create_getter = all_getter;
            let mut default_expr = None;
            let mut some_pattern = None;
            get::attrs(&field.attrs, &["proxy"], |meta| {
                Ok(match_parsed_attr! {
                    meta;
                    option => :is_option,
                    getter => :create_getter,
                    default => default_expr = Some(get::list_or_name_value(meta.input, Ok)?),
                    some_pattern => some_pattern = Some(get::list_or_name_value(meta.input, get::path_from_expr("some_pattern"))?),
                })
            })?;

            let ty = get::unwrapped_ty(&field.ty);
            let doc = field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("doc"));

            if !create_getter {
                return Ok(TokenStream::default());
            }

            if !is_option {
                return Ok(if let Some(ident) = &field.ident {
                    emit_no_option(ident, ty, doc, ident)
                } else {
                    let ident = ::quote::format_ident!("_{i}");
                    emit_no_option(&ident, ty, doc, i)
                });
            }

            Ok(if let Some(ty) = option_ty(ty) {
                let default_expr = default_expr
                    .unwrap_or_else(|| parse_quote!(::core::default::Default::default()));
                let some_pattern = some_pattern.unwrap_or_else(|| parse_quote!(Some));
                if let Some(ident) = &field.ident {
                    emit_option(ident, ty, doc, ident, &default_expr, &some_pattern)
                } else {
                    let ident = ::quote::format_ident!("_{i}");
                    emit_option(&ident, ty, doc, i, &default_expr, &some_pattern)
                }
            } else {
                TokenStream::default()
            })
        })
        .collect::<::syn::Result<TokenStream>>()?;

    let ident = &item.ident;
    let vis = item.vis;
    let doc = format!("Proxy object for [{ident}]");

    let mut proxy_name_ = None;
    let [inner, outer] = proxy_name
        .ok_or_else(|| format_ident!("__Proxy"))
        .map_either(|proxy_name| {
            let proxy = quote! {
                #[doc = #doc]
                #[repr(transparent)]
                #[automatically_derived]
                #vis struct #proxy_name(#ident);
            };
            proxy_name_ = Some(proxy_name);
            proxy
        })
        .split_result();
    let proxy_name = proxy_name_.expect("should have been set by map_either");

    let deref = deref_to_proxy.then(|| {
        quote! {
            #[automatically_derived]
            impl ::core::ops::Deref for #ident {
                type Target = #proxy_name;

                #[inline]
                fn deref(&self) -> &Self::Target {
                    <#ident as #crate_path::Proxy>::proxy(self)
                }
            }
        }
    });

    let as_ref = as_ref_proxy.then(|| {
        quote! {
            #[automatically_derived]
            impl ::core::convert::AsRef<#proxy_name> for #ident {
                #[inline]
                fn as_ref(&self) -> &#proxy_name {
                    <#ident as #crate_path::Proxy>::proxy(self)
                }
            }
        }
    });

    Ok(quote! {
        #outer
        const _: () = {
            #inner
            #deref
            #as_ref

            #[automatically_derived]
            impl #proxy_name {
                #getters
            }

            #[automatically_derived]
            impl ::core::convert::AsRef<#ident> for #proxy_name {
                #[inline]
                fn as_ref(&self) -> &#ident {
                    &self.0
                }
            }

            #[automatically_derived]
            impl #crate_path::Proxy for #ident {
                type Proxy = #proxy_name;

                fn proxy(&self) -> &Self::Proxy {
                    unsafe { &*(self as *const _ as *const _) }
                }
            }
        };
    })
}

/// Get type of an option type.
fn option_ty(ty: &::syn::Type) -> Option<&::syn::Type> {
    let ::syn::Type::Path(ty_path) = ty else {
        push_soft_err(::syn::Error::new_spanned(ty, "expected a type path"));
        return None;
    };

    let Some(::syn::PathArguments::AngleBracketed(arguments)) = ty_path
        .path
        .segments
        .last()
        .map(|segment| &segment.arguments)
    else {
        push_soft_err(::syn::Error::new_spanned(
            ty_path,
            "expected a type argument",
        ));
        return None;
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
        return None;
    };

    Some(ty)
}

/// Emit getter for option fields.
fn emit_option(
    ident: &Ident,
    ty: &::syn::Type,
    doc: impl Iterator<Item = impl ToTokens>,
    acc: impl ToTokens,
    default_expr: &::syn::Expr,
    some_pattern: &::syn::Path,
) -> TokenStream {
    quote! {
        #(#doc)*
        pub fn #ident(&self) -> &#ty {
            static DEFAULT: ::std::sync::OnceLock<#ty> = ::std::sync::OnceLock::new();
            if let #some_pattern(value) = &self.0.#acc {
                value
            } else {
                DEFAULT.get_or_init(|| #default_expr)
            }
        }
    }
}

/// Emit getter for option fields.
fn emit_no_option(
    ident: &Ident,
    ty: &::syn::Type,
    doc: impl Iterator<Item = impl ToTokens>,
    acc: impl ToTokens,
) -> TokenStream {
    quote! {
        #(#doc)*
        pub fn #ident(&self) -> &#ty {
            &self.0.#acc
        }
    }
}
