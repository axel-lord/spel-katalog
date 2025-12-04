//! Implementation for `Delta` derive macro.

use ::std::borrow::Cow;

use ::convert_case::{Case, Casing, ccase};
use ::proc_macro2::{Span, TokenStream};
use ::quote::{ToTokens, format_ident, quote};
use ::syn::Ident;

use crate::{
    ext::{BoolExt, ResultExt},
    get::{self, attrl, match_parsed_attr},
};

/// Implement `Proxy` for a struct.
pub fn fields(item: ::syn::ItemStruct) -> ::syn::Result<::proc_macro2::TokenStream> {
    let mut all_option = false;
    let mut all_skip = false;
    let mut into_fields_name = None;
    let mut fields_ref_name = None;
    let mut fields_mut_name = None;

    let crate_path = get::crate_path_and(&item.attrs, attrl![fields], |meta| {
        Ok(match_parsed_attr! {
            meta;
            skip => :all_skip,
            option => :all_option,
            fields_name => into_fields_name = Some(get::list_or_name_value(meta.input, get::ident_from_expr("fields_name"))?),
            fields_name_ref => fields_ref_name = Some(get::list_or_name_value(meta.input, get::ident_from_expr("fields_name_ref"))?),
            fields_name_mut => fields_mut_name = Some(get::list_or_name_value(meta.input, get::ident_from_expr("fields_name_mut"))?),

        })
    })?;

    let ident = &item.ident;
    let variance = get::xor_hash((
        &into_fields_name,
        &fields_ref_name,
        &fields_mut_name,
        &item.ident,
        item.fields.len(),
    ));

    let is_fields_public = into_fields_name.is_some();
    let is_fields_ref_public = fields_ref_name.is_some();
    let is_fields_mut_public = fields_mut_name.is_some();
    let into_fields_name =
        into_fields_name.unwrap_or_else(|| format_ident!("__{ident}IntoField{variance}"));
    let fields_ref_name =
        fields_ref_name.unwrap_or_else(|| format_ident!("__{ident}FieldsRef{variance}"));
    let fields_mut_name =
        fields_mut_name.unwrap_or_else(|| format_ident!("__{ident}FieldsMut{variance}"));

    let mut fields_variants = TokenStream::default();
    let mut fields_ref_variants = TokenStream::default();
    let mut fields_mut_variants = TokenStream::default();
    let mut apply_arms = TokenStream::default();
    let mut field_count = 0usize;
    let mut field_names = Vec::new();
    let mut variant_names = Vec::new();

    let lt_name = ident.to_string().to_case(Case::Snake);
    let lt = ::syn::Lifetime::new(&format!("'__{lt_name}_{variance}"), Span::call_site());

    item.fields
        .iter()
        .enumerate()
        .try_for_each::<_, ::syn::Result<_>>(|(i, field)| {
            let mut skip = all_skip;
            let mut option = all_option;

            get::attrs(&field.attrs, attrl![fields], |meta| {
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

            let doc = format!("Variant for the {member} field");

            fields_variants.extend(quote! {
                #[doc = #doc]
                #variant_name(#ty),
            });

            fields_ref_variants.extend(quote! {
                #[doc = #doc]
                #variant_name(&#lt #ty),
            });

            fields_mut_variants.extend(quote! {
                #[doc = #doc]
                #variant_name(&#lt mut #ty),
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
    let doc = format!(
        "[IntoFields::Field][{}::IntoFields::Field] enum for {ident}",
        crate_path.to_token_stream()
    );
    let [into_fields_outer, into_fields_inner] = is_fields_public
        .to_result()
        .map_either(|_| {
            quote! {
                #[doc = #doc]
                #[automatically_derived]
                #vis enum #into_fields_name {
                    #fields_variants
                }
            }
        })
        .split_result();

    let [fields_ref_outer, fields_ref_inner] = is_fields_ref_public
        .to_result()
        .map_either(|_| {
            quote! {
                #[doc = #doc]
                #[automatically_derived]
                #vis enum #fields_ref_name<#lt> {
                    #fields_ref_variants
                }
            }
        })
        .split_result();

    let [fields_mut_outer, fields_mut_inner] = is_fields_mut_public
        .to_result()
        .map_either(|_| {
            quote! {
                #[doc = #doc]
                #[automatically_derived]
                #vis enum #fields_mut_name<#lt> {
                    #fields_mut_variants
                }
            }
        })
        .split_result();

    Ok(quote! {
        #fields_ref_outer
        #fields_mut_outer
        #into_fields_outer
        const _: () = {
            #fields_ref_inner
            #fields_mut_inner
            #into_fields_inner

            #[automatically_derived]
            impl #crate_path::IntoFields for #ident {
                type Field = #into_fields_name;
                type IntoFields = [Self::Field; #field_count];

                fn into_fields(self) -> Self::IntoFields {
                    let Self { #(#field_names),* } = self;
                    [#(#into_fields_name::#variant_names(#field_names)),*]
                }
            }

            #[automatically_derived]
            impl<#lt> #crate_path::IntoFields for &#lt #ident {
                type Field = #fields_ref_name<#lt>;
                type IntoFields = [Self::Field; #field_count];

                fn into_fields(self) -> Self::IntoFields {
                    let #ident { #(#field_names),* } = self;
                    [#(#fields_ref_name::#variant_names(#field_names)),*]
                }
            }

            #[automatically_derived]
            impl<#lt> #crate_path::IntoFields for &#lt mut #ident {
                type Field = #fields_mut_name<#lt>;
                type IntoFields = [Self::Field; #field_count];

                fn into_fields(self) -> Self::IntoFields {
                    let #ident { #(#field_names),* } = self;
                    [#(#fields_mut_name::#variant_names(#field_names)),*]
                }
            }

            #[automatically_derived]
            impl #crate_path::FieldDelta for #ident {
                fn delta(&mut self, delta: Self::Field) {
                    match delta {
                        #apply_arms
                    }
                }
            }
        };
    })
}
