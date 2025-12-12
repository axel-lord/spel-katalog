//! ToTokens implementors for some types.

use ::core::fmt::Display;

use ::proc_macro2::TokenStream;
use ::quote::ToTokens;

/// Member access referencing ident.
#[derive(Debug, Clone, Copy)]
pub enum MemberRef<'i> {
    /// Access using identifier.
    Ident(&'i ::syn::Ident),
    /// Access using index.
    Idx(usize),
}

impl<'i> MemberRef<'i> {
    /// If `ident` is some reference it, otherwise use `idx`.
    pub fn from_ident_or(ident: Option<&'i ::syn::Ident>, idx: usize) -> Self {
        Self::from(ident.ok_or(idx))
    }
}

impl Display for MemberRef<'_> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        match self {
            MemberRef::Ident(ident) => Display::fmt(ident, f),
            MemberRef::Idx(idx) => Display::fmt(idx, f),
        }
    }
}

impl<'i> From<Result<&'i ::syn::Ident, usize>> for MemberRef<'i> {
    fn from(value: Result<&'i ::syn::Ident, usize>) -> Self {
        match value {
            Ok(ident) => Self::Ident(ident),
            Err(idx) => Self::Idx(idx),
        }
    }
}

impl ToTokens for MemberRef<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            MemberRef::Ident(ident) => ident.to_tokens(tokens),
            MemberRef::Idx(idx) => idx.to_tokens(tokens),
        }
    }
    fn to_token_stream(&self) -> TokenStream {
        match self {
            MemberRef::Ident(ident) => ident.to_token_stream(),
            MemberRef::Idx(idx) => idx.to_token_stream(),
        }
    }
    fn into_token_stream(self) -> TokenStream
    where
        Self: Sized,
    {
        match self {
            MemberRef::Ident(ident) => ident.into_token_stream(),
            MemberRef::Idx(idx) => idx.into_token_stream(),
        }
    }
}
