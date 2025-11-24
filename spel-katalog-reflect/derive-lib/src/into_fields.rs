//! Implementation for `Delta` derive macro.

use ::std::borrow::Cow;

use ::convert_case::ccase;
use ::proc_macro2::TokenStream;
use ::quote::{ToTokens, format_ident, quote};
use ::syn::{Ident, parse::Parse};

use crate::{
    ext::{BoolExt, ResultExt},
    get::{self, attrl, match_parsed_attr},
};

/// Implement `Proxy` for a struct.
pub fn into_fields(item: ::syn::ItemStruct) -> ::syn::Result<::proc_macro2::TokenStream> {
    let mut all_option = false;
    let mut all_skip = false;
    let mut into_fields_name = None;

    let crate_path = get::crate_path_and(&item.attrs, attrl![into_fields], |meta| {
        Ok(match_parsed_attr! {
            meta;
            skip => :all_skip,
            option => :all_option,
            fields_name => into_fields_name = Some(get::list_or_name_value(meta.input, Ident::parse)?),
        })
    })?;

    let is_non_anon = into_fields_name.is_some();
    let into_fields_name = into_fields_name.unwrap_or_else(|| format_ident!("__IntoField"));

    let mut delta_variants = TokenStream::default();
    let mut apply_arms = TokenStream::default();
    let mut field_count = 0usize;
    let mut field_names = Vec::new();
    let mut variant_names = Vec::new();

    item.fields
        .iter()
        .enumerate()
        .try_for_each::<_, ::syn::Result<_>>(|(i, field)| {
            let mut skip = all_skip;
            let mut option = all_option;

            get::attrs(&field.attrs, attrl![into_fields], |meta| {
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

            let field_name = field
                .ident
                .as_ref()
                .map_or_else(|| Cow::Owned(format_ident!("_{i}")), Cow::Borrowed);

            let ty = get::unwrapped_ty(&field.ty);

            let doc = format!("Delta variant for the {member} field");

            delta_variants.extend(quote! {
                #[doc = #doc]
                #variant_name(#ty),
            });

            apply_arms.extend(quote! {
                #into_fields_name::#variant_name(value) => self.#member = value,
            });

            field_count += 1;

            field_names.push(field_name);
            variant_names.push(variant_name);

            Ok(())
        })?;

    let vis = &item.vis;
    let ident = &item.ident;
    let doc = format!(
        "[IntoFields::Field][{}::IntoFields::Field] enum for {ident}",
        crate_path.to_token_stream()
    );
    let [outer, inner] = is_non_anon
        .to_result()
        .map_either(|_| {
            quote! {
                #[doc = #doc]
                #vis enum #into_fields_name {
                    #delta_variants
                }
            }
        })
        .split_result();

    Ok(quote! {
        #outer
        const _: () = {
            #inner
            impl #crate_path::IntoFields for #ident {
                type Field = #into_fields_name;
                type IntoFields = [Self::Field; #field_count];

                fn into_fields(self) -> Self::IntoFields {
                    let Self { #(#field_names),* } = self;
                    [#(#into_fields_name::#variant_names(#field_names)),*]
                }

                fn delta(&mut self, delta: Self::Field) {
                    match delta {
                        #apply_arms
                    }
                }
            }
        };
    })
}
