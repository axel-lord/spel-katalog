//! Implementation for `Delta` derive macro.

use ::convert_case::ccase;
use ::proc_macro2::TokenStream;
use ::quote::{ToTokens, format_ident, quote};
use ::syn::{Ident, parse::Parse, parse_quote};

use crate::get::{self, attrl, match_parsed_attr};

/// Implement `Proxy` for a struct.
pub fn delta(item: ::syn::ItemStruct) -> ::syn::Result<::proc_macro2::TokenStream> {
    let mut all_option = false;
    let mut all_skip = false;
    let mut delta_name = None;

    let crate_path = get::crate_path_and(&item.attrs, attrl![delta], |meta| {
        Ok(match_parsed_attr! {
            meta;
            skip => :all_skip,
            option => :all_option,
            delta_name => delta_name = Some(get::list_or_name_value(meta.input, Ident::parse)?),
        })
    })?;

    let delta_name = delta_name.unwrap_or_else(|| format_ident!("__Delta"));

    let mut delta_variants = TokenStream::default();
    let mut apply_arms = TokenStream::default();

    item.fields
        .iter()
        .enumerate()
        .try_for_each::<_, ::syn::Result<_>>(|(i, field)| {
            let mut skip = all_skip;
            let mut option = all_option;

            get::attrs(&field.attrs, attrl![delta], |meta| {
                Ok(match_parsed_attr! {
                    meta;
                    skip => :skip,
                    option => :option,
                })
            })?;

            if skip {
                return Ok(());
            }

            let variant_name = field.ident.as_ref().map_or_else(
                || Ok(format_ident!("_{i}")),
                |ident| -> ::syn::Result<_> {
                    let name = ident.to_string();
                    let mut name = if let Some(ident) = name.strip_prefix("r#") {
                        let name = ccase!(pascal, ident);
                        ::syn::parse_str::<Ident>(&format!("r#{name}"))?
                    } else {
                        let name = ccase!(pascal, name);
                        syn::parse_str::<Ident>(&name)?
                    };
                    name.set_span(ident.span());

                    Ok(name)
                },
            )?;

            let member = field
                .ident
                .as_ref()
                .map_or_else(|| i.to_token_stream(), |ident| ident.to_token_stream());

            let ty = get::unwrapped_ty(&field.ty);

            let doc = format!("Delta variant for the {member} field");

            delta_variants.extend(quote! {
                #[doc = #doc]
                #variant_name(#ty),
            });

            apply_arms.extend(quote! {
                #delta_name::#variant_name(value) => self.#member = value,
            });

            Ok(())
        })?;

    Ok(quote! {
        const _:() {

        }
    })
}
