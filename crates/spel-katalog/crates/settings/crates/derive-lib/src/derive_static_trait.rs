//! Derive static traits.

use ::proc_macro2::{Span, TokenStream};
use ::quote::quote;
use ::syn::{Attribute, Ident, Visibility, parse::ParseStream};

use crate::util;

/// Derive a trait with a single method returning a static value.
pub(crate) fn derive_static_trait(
    input: ParseStream,
    trait_name: &str,
    attr_name: &str,
    method_name: &str,
) -> ::syn::Result<TokenStream> {
    let attrs = input.call(Attribute::parse_outer)?;
    input.parse::<Visibility>()?;
    input.call(util::parse_any_type_class)?;
    let ident = input.parse::<Ident>()?;
    input.parse::<TokenStream>()?;

    let mut expr_attr = None;

    util::parse_settings_attr(&attrs, attr_name, |meta| {
        let name_value = meta.require_name_value()?;
        expr_attr = Some(name_value.value.clone());
        Ok(())
    })?;

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
