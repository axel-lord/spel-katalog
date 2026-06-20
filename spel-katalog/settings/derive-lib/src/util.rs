//! Utility functions

use ::syn::{
    Attribute, Meta, Token,
    parse::{ParseStream, Parser},
    punctuated::Punctuated,
};

/// Parse any type class (struct, enum, union) and throw away the result
pub(crate) fn parse_any_type_class(input: ParseStream) -> ::syn::Result<()> {
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

/// Parse a specific attr in the set of settings attrs.
pub fn parse_settings_attr(
    attrs: &[Attribute],
    name: &str,
    mut f: impl FnMut(&Meta) -> ::syn::Result<()>,
) -> ::syn::Result<()> {
    for attr in attrs {
        if attr.path().is_ident(name) {
            f(&attr.meta)?;
        } else if attr.path().is_ident("settings") {
            Punctuated::<Meta, Token![,]>::parse_terminated
                .parse2(attr.meta.require_list()?.tokens.clone())?
                .iter()
                .filter(|meta| meta.path().is_ident(name))
                .try_for_each(&mut f)?;
        }
    }
    Ok(())
}
