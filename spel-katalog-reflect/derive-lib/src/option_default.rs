//! Implementation for `OptionDefault` derive macro.

use ::core::ops::ControlFlow;

use ::proc_macro2::TokenStream;
use ::quote::{ToTokens, format_ident, quote};
use ::syn::{Ident, parse::Parse, parse_quote};

use crate::{get, soft_err::push_soft_err};

/// Implement `OptDefault` for a struct.
pub fn option_default(item: ::syn::ItemStruct) -> ::syn::Result<TokenStream> {
    let mut all_option = false;
    let mut proxy_name = None;
    let crate_path = get::crate_path_and(&item.attrs, "option_default", |meta| {
        Ok(if meta.path.is_ident("option") {
            all_option = true;
            ControlFlow::Break(())
        } else if meta.path.is_ident("no_option") {
            all_option = false;
            ControlFlow::Break(())
        } else if meta.path.is_ident("proxy_name") {
            proxy_name = Some(get::list_or_name_value(meta.input, Ident::parse)?);
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
            let mut default_expr = None;
            let mut some_pattern = None;
            get::attrs(&field.attrs, "option_default", |meta| {
                Ok(if meta.path.is_ident("option") {
                    is_option = true;
                    ControlFlow::Break(())
                } else if meta.path.is_ident("no_option") {
                    is_option = false;
                    ControlFlow::Break(())
                } else if meta.path.is_ident("default") {
                    default_expr = Some(get::list_or_name_value(meta.input, ::syn::Expr::parse)?);
                    ControlFlow::Break(())
                } else if meta.path.is_ident("some_pattern") {
                    some_pattern = Some(get::list_or_name_value(meta.input, ::syn::Path::parse)?);
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
    let vis = item.vis;
    let doc = format!("Proxy object for [{ident}]");

    let (inner_outer, proxy_name) = if let Some(proxy_name) = proxy_name {
        ([false, true], proxy_name)
    } else {
        ([true, false], format_ident!("__Proxy"))
    };

    let [inner, outer] = inner_outer.map(|exists| {
        exists.then(|| {
            quote! {
                #[derive(Clone, Copy)]
                #[doc = #doc]
                #vis struct #proxy_name<'__this>(&'__this #ident);
            }
        })
    });

    Ok(quote! {
        #outer
        const _: () = {
            #inner

            impl<'__this> #proxy_name<'__this> {
                #getters
            }

            impl ::core::ops::Deref for #proxy_name<'_> {
                type Target = #ident;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl ::core::convert::AsRef<#ident> for #proxy_name<'_> {
                fn as_ref(&self) -> &#ident {
                    &self.0
                }
            }

            impl #crate_path::OptionDefault for #ident {
                type Proxy<'__this> = #proxy_name<'__this>;

                fn proxy(&self) -> Self::Proxy<'_> {
                    #proxy_name(self)
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
        pub fn #ident(self) -> &'__this #ty {
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
        pub fn #ident(self) -> &'__this #ty {
            &self.0.#acc
        }
    }
}
