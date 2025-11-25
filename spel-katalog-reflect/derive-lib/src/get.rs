//! utility functions.

use ::core::ops::{ControlFlow, Deref};
use ::std::borrow::Cow;

use ::proc_macro2::TokenTree;
use ::syn::{
    Attribute, Fields, Ident, Token, meta::ParseNestedMeta, parenthesized, parse::ParseStream,
    parse_quote, parse_quote_spanned,
};

use crate::soft_err::push_soft_err;

/// Create a list of attr names from idents.
///
/// ```ignore
/// assert_eq!(attrl![a b v], &["a", "b", "c"]);
/// assert_eq!(attrl![a], &["a"]);
/// ```
macro_rules! attrl {
    ($($ident:ident)*) => {
        &[$(stringify!($ident)),*]
    };
}
pub(crate) use attrl;

/// Match on attribute names.
///
/// ```ignore
/// let mut is_cool = false;
/// let mut is_kind = false;
/// let mut is_mean = false;
/// match_parsed_attr! {
///     // Match on an attribute.
///     "cool" => is_cool = true,
///     // Ident match.
///     kind => is_kind = true,
///     // Flag match same as above with an additional "no_mean"
///     // match doing the opposite.
///     "mean" => :is_mean,
/// }?;
///
/// println!("neither cool nor kind nor mean");
///
/// ```
macro_rules! match_parsed_attr {
    // Match ident flag.
    (@arm $($flag:ident)+ $block:lifetime, $meta:expr, $name:ident) => {
        $crate::get::match_parsed_attr!(@arm $($flag)* $block, $meta, stringify!($name));
    };
    // Match ident with expr.
    (@arm $block:lifetime, $meta:expr, $name:ident, $expr:expr) => {
        $crate::get::match_parsed_attr!(@arm $block, $meta, stringify!($name), $expr);
    };
    // Match str flag.
    (@arm $($flag:ident)+ $block:lifetime, $meta:expr, $name:expr) => {
        $crate::get::match_parsed_attr!(@arm $block, $meta, $name, { $($flag = true;)* });
        $crate::get::match_parsed_attr!(@arm $block, $meta, concat!("no_", $name), { $($flag = false;)* });
    };
    // Mathc expr.
    (@arm $block:lifetime, $meta:expr, $name:expr, $expr:expr) => {
        if $meta.path.is_ident($name) {
            $expr;
            break $block ::core::ops::ControlFlow::Break(())
        }
    };
    ($meta:expr;
        $($name:tt => $(:$flag:ident)* $($expr:expr)?,)*
    ) => {'block: {
        $( $crate ::get::match_parsed_attr!(@arm $($flag)* 'block, $meta, $name $(, $expr)*); )* {
            ::core::ops::ControlFlow::Continue(())
        }
    }};
}
pub(crate) use match_parsed_attr;

/// Wrapper for `ParseNestedMeta` with additional context.
#[expect(dead_code)]
#[derive(Clone, Copy)]
pub struct ParsedAttr<'a> {
    /// Wrapped nested meta parser.
    pub parse_nested_meta: &'a ParseNestedMeta<'a>,
    /// Name of parsed attribute.
    pub name: &'a str,
    /// If the name is of the global attribute `reflect`.
    pub is_global: bool,
}

impl<'a> Deref for ParsedAttr<'a> {
    type Target = ParseNestedMeta<'a>;

    fn deref(&self) -> &Self::Target {
        self.parse_nested_meta
    }
}

/// Get attributes from attribute list.
pub fn attrs(
    attr_list: &[Attribute],
    attr_name: &[&str],
    mut with: impl FnMut(ParsedAttr) -> ::syn::Result<ControlFlow<()>>,
) -> ::syn::Result<()> {
    let mut attr_parsed = false;
    for attr in attr_list {
        if attr.path().is_ident("reflect") {
            attr.parse_nested_meta(|meta| {
                for attr_name in attr_name {
                    if meta.path.is_ident(attr_name) {
                        meta.parse_nested_meta(|meta| {
                            let result = with(ParsedAttr {
                                parse_nested_meta: &meta,
                                name: attr_name,
                                is_global: false,
                            })?;
                            if result.is_continue() {
                                push_soft_err(meta.error("unsupported property"));
                            }
                            Ok(())
                        })?;
                        return Ok(());
                    }
                }

                let result = with(ParsedAttr {
                    parse_nested_meta: &meta,
                    name: "reflect",
                    is_global: true,
                })?;

                if result.is_continue() {
                    meta.input.step(|cursor| {
                        let mut remainder = *cursor;
                        while let Some((tt, next)) = remainder.token_tree() {
                            if let TokenTree::Punct(punct) = tt
                                && punct.as_char() == ','
                            {
                                return Ok(((), remainder));
                            };

                            remainder = next;
                        }
                        Ok(((), remainder))
                    })?;
                }

                Ok(())
            })?;
            if attr_parsed {
                push_soft_err(::syn::Error::new_spanned(
                    attr.path(),
                    format!("reflect attribute should not be placed after {attr_name:?} attribute"),
                ));
            }
        } else {
            for attr_name in attr_name {
                if attr.path().is_ident(attr_name) {
                    attr.parse_nested_meta(|meta| {
                        let result = with(ParsedAttr {
                            parse_nested_meta: &meta,
                            name: attr_name,
                            is_global: false,
                        })?;
                        if result.is_continue() {
                            push_soft_err(meta.error("unsupported property"));
                        }
                        Ok(())
                    })?;
                    attr_parsed = true;
                }
            }
        }
    }
    Ok(())
}

/// Get crate_path attribute.
pub fn crate_path(attrs: &[Attribute], attr_name: &[&str]) -> ::syn::Result<::syn::ExprPath> {
    crate_path_and(attrs, attr_name, |_| Ok(ControlFlow::Continue(())))
}

/// Get top level attributes. returning crate_path attributes and allowing a closure to be ran on
/// other attributes.
pub fn crate_path_and(
    attr_list: &[Attribute],
    attr_name: &[&str],
    mut with: impl FnMut(ParsedAttr) -> ::syn::Result<ControlFlow<()>>,
) -> ::syn::Result<::syn::ExprPath> {
    let mut crate_path = None;
    attrs(attr_list, attr_name, |meta| {
        let parse_crate_path = |tokens: ParseStream| -> ::syn::Result<Option<::syn::ExprPath>> {
            match tokens.parse::<::syn::Expr>()? {
                ::syn::Expr::Path(path) => Ok(Some(path)),
                _ => Err(meta.error("crate_path must be a module path")),
            }
        };
        if meta.path.is_ident("crate_path") {
            crate_path = list_or_name_value(meta.input, parse_crate_path)?;
            Ok(ControlFlow::Break(()))
        } else {
            with(meta)
        }
    })?;
    Ok(crate_path.unwrap_or_else(|| parse_quote!(::spel_katalog_reflect)))
}

/// Parse `name = value` or `list()`, content.
pub fn list_or_name_value<T>(
    stream: ParseStream,
    parser: impl FnOnce(ParseStream) -> ::syn::Result<T>,
) -> ::syn::Result<T> {
    if stream.peek(Token![=]) {
        _ = stream.parse::<Token![=]>()?;
        parser(stream)
    } else {
        let content;
        parenthesized!(content in stream);
        parser(&content)
    }
}

/// Get variants as string literals, using as_str attribute if avaialable.
pub fn variants_as_str_reprs(item: &::syn::ItemEnum) -> ::syn::Result<Vec<Cow<'_, ::syn::LitStr>>> {
    /// Get variant as a string literal, using as_str attribute if avaialable.
    fn get_variant_as_str(variant: &::syn::Variant) -> ::syn::Result<Cow<'_, ::syn::LitStr>> {
        let mut str_rep = None;
        for attr in &variant.attrs {
            let Some(ident) = attr.path().get_ident() else {
                continue;
            };

            if ident != "as_str" {
                continue;
            }

            let value = match &attr.meta {
                ::syn::Meta::Path(path) => {
                    return Err(::syn::Error::new_spanned(
                        path,
                        "as_str property must be of the 'as_str = _' or 'as_str(_)' format",
                    ));
                }
                ::syn::Meta::List(meta_list) => Cow::Owned(meta_list.parse_args()?),
                ::syn::Meta::NameValue(meta_name_value) => match &meta_name_value.value {
                    ::syn::Expr::Lit(::syn::ExprLit {
                        lit: ::syn::Lit::Str(lit_str),
                        ..
                    }) => Cow::Borrowed(lit_str),
                    other => {
                        return Err(::syn::Error::new_spanned(
                            other,
                            "as_str propery must have a string literal value",
                        ));
                    }
                },
            };
            str_rep = Some(value);
        }

        Ok(if let Some(str_rep) = str_rep {
            str_rep
        } else {
            let str_rep = variant.ident.to_string();
            Cow::Owned(parse_quote_spanned!(variant.ident.span()=> #str_rep))
        })
    }
    item.variants.iter().map(get_variant_as_str).collect()
}

/// Get idents of fields of enum if they are all unit fields.
pub fn unit_variants(item: &::syn::ItemEnum) -> ::syn::Result<Vec<&Ident>> {
    item.variants
        .iter()
        .map(|variant| {
            if !matches!(variant.fields, Fields::Unit) {
                Err(::syn::Error::new_spanned(
                    &variant.fields,
                    "only unit variants expected",
                ))
            } else {
                Ok(&variant.ident)
            }
        })
        .collect()
}

/// Unwrap a syn type.
pub fn unwrapped_ty(ty: &::syn::Type) -> &::syn::Type {
    let mut ty = ty;
    loop {
        match ty {
            ::syn::Type::Group(::syn::TypeGroup { elem, .. })
            | ::syn::Type::Paren(::syn::TypeParen { elem, .. }) => {
                ty = elem;
            }
            ty => return ty,
        }
    }
}
