//! Functions generating item proxies.

use ::quote::ToTokens;
use ::syn::{Expr, File, Item, Stmt, Token, Type, parse_quote, punctuated::Punctuated};

macro_rules! spec {
    {$($ident:ident: $ty:ty),* $(,)?} => {
        $( let $ident: $ty = parse_quote!(#$ident); )*
    };
}

/// Create a syn file with the given items.
pub fn file(items: &mut dyn Iterator<Item = Item>) -> File {
    File {
        shebang: None,
        attrs: Vec::new(),
        items: items.collect(),
    }
}

/// Generate a deref implementation for the given type with the given target and body.
pub fn deref(ty: &dyn ToTokens, target: &dyn ToTokens, body: &dyn ToTokens) -> Item {
    spec!(ty: Type, target: Type, body: Vec<Stmt>);
    parse_quote! {
        impl ::core::ops::Deref for #ty {
            type Target = #target;
            fn deref(&self) -> &Self::Target {
                #( #body )*
            }
        }
    }
}

/// Generate a default implementation for the given type with the given body.
pub fn default(ty: &dyn ToTokens, body: &dyn ToTokens) -> Item {
    spec!(ty: Type, body: Vec<Stmt>);
    parse_quote! {
        impl ::core::default::Default for #ty {
            fn default() -> Self {
                #( #body )*
            }
        }
    }
}

/// Generate a display implementation for the given type with the given body.
pub fn display(ty: &dyn ToTokens, body: &dyn ToTokens) -> Item {
    spec!(ty: Type, body: Vec<Stmt>);
    parse_quote! {
        impl ::core::fmt::Display for #ty {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                #( #body )*
            }
        }
    }
}

/// Generate an AsRef implementation for the given type with the given target and body.
pub fn as_ref(ty: &dyn ToTokens, target: &dyn ToTokens, body: &dyn ToTokens) -> Item {
    spec!(ty: Type, target: Type, body: Vec<Stmt>);
    parse_quote! {
        impl ::core::convert::AsRef<#target> for #ty {
            fn as_ref(&self) -> &#target {
                #( #body )*
            }
        }
    }
}

/// Generate a from implementation for the given type with the given target and body.
pub fn from(ty: &dyn ToTokens, from: &dyn ToTokens, body: &dyn ToTokens) -> Item {
    spec!(ty: Type, from: Type, body: Vec<Stmt>);
    parse_quote! {
        impl ::core::convert::From<#from> for #ty {
            fn from(value: #from) -> Self {
                #( #body )*
            }
        }
    }
}

/// Generate a Title implementation for the given type with the given body.
pub fn title(ty: &dyn ToTokens, body: &dyn ToTokens) -> Item {
    spec!(ty: Type, body: Vec<Stmt>);
    parse_quote! {
        impl crate::Title for #ty {
            fn title() -> &'static str {
                #( #body )*
            }
        }
    }
}

/// Generate a DefaultStr implementation for the given type with the given body.
pub fn default_str(ty: &dyn ToTokens, body: &dyn ToTokens) -> Item {
    spec!(ty: Type, body: Vec<Stmt>);
    parse_quote! {
        impl crate::DefaultStr for #ty {
            fn default_str() -> &'static str {
                #( #body )*
            }
        }
    }
}

/// Generate a Variants implementation for the given type with the given variants.
pub fn variants(ty: &dyn ToTokens, variants: &dyn ToTokens) -> Item {
    spec!(ty: Type, variants: Punctuated<Expr, Token![,]>);
    parse_quote! {
        unsafe impl crate::Variants for #ty {
            const VARIANTS: &[Self] = &[#variants];
        }
    }
}
