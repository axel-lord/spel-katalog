//! Implementation for `OptionDefault` derive macro.

use ::core::ops::ControlFlow;

use ::proc_macro2::TokenStream;
use ::quote::{ToTokens, quote};

use crate::{get, soft_err::push_soft_err};

/// Implement `OptDefault` for a struct.
pub fn option_default(item: ::syn::ItemStruct) -> ::syn::Result<TokenStream> {
    let mut all_option = false;
    let _crate_path = get::crate_path_and(&item.attrs, "option_default", |meta| {
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
            get::attrs(&field.attrs, "option_default", |meta| {
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

            let ty = get::unwrapped_ty(&field.ty);
            let doc = field
                .attrs
                .iter()
                .filter(|attr| attr.path().is_ident("doc"));

            Ok(if is_option {
                if let Some(ty) = option_ty(ty) {
                    if let Some(ident) = &field.ident {
                        emit_option(ident, ty, doc, ident)
                    } else {
                        let ident = ::quote::format_ident!("_{i}");
                        emit_option(&ident, ty, doc, i)
                    }
                } else {
                    TokenStream::default()
                }
            } else if let Some(ident) = &field.ident {
                emit_no_option(ident, ty, doc, ident)
            } else {
                let ident = ::quote::format_ident!("_{i}");
                emit_no_option(&ident, ty, doc, i)
            })
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
    ident: &::syn::Ident,
    ty: &::syn::Type,
    doc: impl Iterator<Item = impl ToTokens>,
    acc: impl ToTokens,
) -> TokenStream {
    quote! {
        #(#doc)*
        pub fn #ident(self) -> &'__this #ty {
            static DEFAULT: ::std::sync::OnceLock<#ty> = ::std::sync::OnceLock::new();
            if let Some(value) = &self.0.#acc {
                value
            } else {
                DEFAULT.get_or_init(|| ::core::default::Default::default())
            }
        }
    }
}

/// Emit getter for option fields.
fn emit_no_option(
    ident: &::syn::Ident,
    ty: &::syn::Type,
    doc: impl Iterator<Item = impl ToTokens>,
    acc: impl ToTokens,
) -> TokenStream {
    quote! {
        #(#doc)*
        pub fn #ident(self) -> &'__this #ty {
            &self.0.#acc
        }
    }
}
