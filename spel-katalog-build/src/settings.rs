//! Settings writing.

use ::std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use ::convert_case::{Case, Casing};
use ::quote::{format_ident, quote};
use ::syn::{Ident, parse_quote};

use crate::{
    expr::{str_expr, title_expr},
    format::{
        Settings,
        settings::{Setting, SettingContent},
    },
    item,
    string::doc_str,
};

struct Emit {
    ty: ::syn::Item,
    impls: ::syn::File,
    from_str: ::syn::File,
}

fn emit_enum(
    setting: &Setting,
    name: &str,
    ident: &Ident,
    variants: &[String],
    default: &str,
) -> Emit {
    let doc = doc_str(&setting.help);
    let (variant_idents, is_variants, is_variants_doc) = variants
        .iter()
        .map(|variant| {
            (
                format_ident!("{}", variant.to_case(Case::Pascal)),
                format_ident!("is_{}", variant.to_case(Case::Snake)),
                format!(
                    "Check if setting is of the `{}` variant.",
                    variant.to_case(Case::Pascal)
                ),
            )
        })
        .collect::<(Vec<_>, Vec<_>, Vec<_>)>();
    let uppercase_variants = variants.iter().map(|v| v.to_uppercase());
    let default_ident = format_ident!("{}", default.to_case(Case::Pascal));
    let title_body = title_expr(name, setting.title.as_deref());

    let parse_err_ident = format_ident!("Parse{ident}Error");
    let parse_err_doc = format!(
        "Error used by [FromStr][::core::str::FromStr] implementation for [{name}]",
        name = name.to_case(Case::Pascal)
    );
    let parse_err_msg = format!("string not one of {}", variants.join(", "));

    // impls
    let impls = item::file(
        &mut [
            parse_quote! {
                impl #ident {
                    #[doc = "Get string representation of current variant."]
                    #[inline]
                    pub fn as_str(&self) -> &str {
                        match self {
                            #( Self::#variant_idents => #variants, )*
                        }
                    }

                    #(
                        #[doc = #is_variants_doc]
                        pub fn #is_variants(&self) -> bool {
                            match self {
                                Self::#variant_idents => true,
                                _ => false,
                            }
                        }
                    )*
                }

            },
            item::default_str(ident, &default),
            item::title(ident, &title_body),
            item::variants(ident, &quote! { #( Self::#variant_idents ),* }),
            item::default(ident, &quote! { Self::#default_ident }),
            item::display(ident, &quote! { f.write_str(self.as_str()) }),
            item::as_ref(ident, &quote! { str }, &quote! { self.as_str() }),
            item::from(
                &quote! { ::std::string::String },
                ident,
                &quote! { value.as_str().into() },
            ),
        ]
        .into_iter(),
    );

    let ty = parse_quote! {
        #[derive(
            Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash,
            ::serde::Deserialize, ::serde::Serialize,
            ::clap::ValueEnum,
        )]
        #[doc = #doc]
        pub enum #ident {
            #(#variant_idents,)*
        }
    };

    let from_str = parse_quote! {
        impl ::core::str::FromStr for #ident {
            type Err = #parse_err_ident;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                #(
                if s.chars().map(|c| c.to_uppercase()).flatten().eq(#uppercase_variants.chars()) {
                    return Ok(Self::#variant_idents);
                }
                )*
                Err(#parse_err_ident)
            }
        }

        #[derive(Debug)]
        #[doc = #parse_err_doc]
        pub struct #parse_err_ident;

        impl ::core::fmt::Display for #parse_err_ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(#parse_err_msg)
            }
        }

        impl ::core::error::Error for #ident {}
    };

    Emit {
        ty,
        impls,
        from_str,
    }
}

fn emit_path(setting: &Setting, name: &str, ident: &Ident, path: &str) -> Emit {
    let title_body = title_expr(name, setting.title.as_deref());
    let doc = doc_str(&setting.help);
    let default_value = str_expr(path);

    let ty = parse_quote! {
        #[derive(
            Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash,
            ::serde::Serialize, ::serde::Deserialize
        )]
        #[doc = #doc]
        #[serde(transparent)]
        pub struct #ident(String);
    };

    let impls = item::file(
        &mut [
            parse_quote! {
                impl #ident {
                    #[doc = "Construct a new value from a string."]
                    #[inline]
                    pub fn new(string: ::std::string::String) -> Self {
                        Self(string)
                    }

                    #[doc = "Unwrap into inner string value."]
                    #[inline]
                    pub fn into_inner(self) -> ::std::string::String {
                        let Self(string) = self;
                        string
                    }

                    #[doc = "Get setting as a string slice."]
                    #[inline]
                    pub fn as_str(&self) -> &str {
                        &self.0
                    }

                    #[doc = "Get setting as a path."]
                    #[inline]
                    pub fn as_path(&self) -> &::std::path::Path {
                        ::std::path::Path::new(self.as_str())
                    }

                    #[doc = "Get setting as an os string."]
                    #[inline]
                    pub fn as_os_str(&self) -> &::std::ffi::OsStr{
                        ::std::ffi::OsStr::new(self.as_str())
                    }

                    #[doc = "Get a `PathBuf` from setting."]
                    #[inline]
                    pub fn to_path_buf(&self) -> ::std::path::PathBuf {
                        self.as_path().to_path_buf()
                    }
                }
            },
            item::title(ident, &title_body),
            item::default_str(ident, &default_value),
            item::default(
                ident,
                &quote! { Self(<Self as crate::DefaultStr>::default_str().into()) },
            ),
            item::display(ident, &quote! { f.write_str(self.as_str()) }),
            item::deref(ident, &quote! { str }, &quote! { self.as_str() }),
            item::as_ref(ident, &quote! { str }, &quote! { self.as_str() }),
            item::as_ref(
                ident,
                &quote! { ::std::path::Path },
                &quote! { self.as_path() },
            ),
            item::as_ref(
                ident,
                &quote! { ::std::ffi::OsStr },
                &quote! { self.as_os_str() },
            ),
            item::from(
                ident,
                &quote! { ::std::string::String },
                &quote! { Self::new(value) },
            ),
            item::from(
                &quote! { ::std::string::String },
                ident,
                &quote! { value.into_inner() },
            ),
        ]
        .into_iter(),
    );

    let from_str = parse_quote! {
        impl ::core::str::FromStr for #ident {
            type Err = ::core::convert::Infallible;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(s.into()))
            }
        }
    };

    Emit {
        ty,
        impls,
        from_str,
    }
}

fn emit_type(setting: &Setting, name: &str) -> Emit {
    let ident = format_ident!("{}", name.to_case(Case::Pascal));
    match &setting.content {
        SettingContent::Enum { variants, default } => {
            emit_enum(setting, name, &ident, variants, default)
        }
        SettingContent::Path { path } => emit_path(setting, name, &ident, path),
    }
}

/// Write settings rust code to destination path.
pub fn write(settings: Settings, dest: &Path) {
    let mut file = BufWriter::new(File::create(dest).unwrap());
    let Settings { settings } = settings;

    let emitted = settings
        .iter()
        .map(|(name, setting)| (name, setting, emit_type(setting, name)))
        .collect::<Vec<_>>();
    // emitted.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

    let from_str = emitted.iter().map(|(.., e)| &e.from_str);
    let types = emitted.iter().map(|(.., e)| &e.ty);
    let impls = emitted.iter().map(|(.., e)| &e.impls);

    let mut ty_names = Vec::new();
    let mut ty_doc = Vec::new();
    let mut is_variants = Vec::new();
    let mut is_variants_docs = Vec::new();
    let mut field_names = Vec::new();
    let mut enum_field_names = Vec::new();
    let mut path_field_names = Vec::new();

    for (name, setting, ..) in &emitted {
        let pascal_ident = name.to_case(Case::Pascal);
        let snake_ident = name.to_case(Case::Snake);
        ty_names.push(format_ident!("{pascal_ident}"));
        ty_doc.push(doc_str(&setting.help));
        is_variants.push(format_ident!("is_{snake_ident}",));
        is_variants_docs.push(format!(
            "Check if delta is of the `{pascal_ident}` variant.",
        ));
        field_names.push(format_ident!("{snake_ident}"));

        match setting.content {
            SettingContent::Enum { .. } => &mut enum_field_names,
            SettingContent::Path { .. } => &mut path_field_names,
        }
        .push(format_ident!("{snake_ident}"));
    }

    file.write_all(
        ::prettyplease::unparse(&parse_quote! {
            /// Error types used for [FromStr][::core::str::FromStr] implementations.
            pub mod error {
                use super::*;
                #(#from_str)*
            }
            #(#types)*
            #(#impls)*

            /// Application settings.
            #[derive(
                Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash,
                ::serde::Deserialize, ::serde::Serialize, ::clap::Args,
            )]
            pub struct Settings {
                #(
                #[doc = #ty_doc]
                #[arg(long)]
                #[serde(skip_serializing_if = "Option::is_none", default)]
                pub(crate) #field_names: Option<#ty_names>,
                )*
            }

            impl Settings {
                pub fn apply(mut self, delta: impl IntoIterator<Item = Delta>) -> Self {
                    for delta in delta {
                        delta.apply(&mut self);
                    }
                    self
                }

                pub fn skeleton(&self) -> Self {
                    Self {#(
                        #field_names: Some(self.#field_names.clone().unwrap_or_default()),
                    )*}
                }

                #(
                pub fn #field_names(&self) -> &#ty_names {
                    static DEFAULT: ::std::sync::OnceLock<#ty_names> = ::std::sync::OnceLock::new();

                    match &self.#field_names {
                        Some(value) => value,
                        None => DEFAULT.get_or_init(Default::default),
                    }
                }
                )*

                pub fn view_enums(&self) -> ::iced::Element<Delta> {
                    crate::list::enum_list([
                        #( crate::list::enum_choice(self.#enum_field_names), )*
                    ]).into()
                }

                pub fn view_paths(&self) -> ::iced::Element<Delta> {
                    crate::list::path_list([
                        #( crate::list::path_choice(self.#path_field_names), )*
                    ]).into()
                }
            }

            /// A Change in a setting.
            #[derive(
                Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash,
                ::serde::Deserialize, ::serde::Serialize
            )]
            pub enum Delta {
                #(
                #[doc = #ty_doc]
                #ty_names(#ty_names),
                )*
            }

            impl Delta {
                /// Apply delta to settings.
                #[inline]
                pub fn apply(self, settings: &mut Settings) -> &mut Settings {
                    match self {
                        #( Self::#ty_names(value) => settings.#field_names = Some(value), )*
                    }
                    settings
                }

                /// Create setting deltas from a [Settings].
                pub fn create(settings: Settings) -> impl Iterator<Item = Delta> {
                    let Settings { #( #field_names ),* } = settings;
                    [#( #field_names.map(|value| Self::#ty_names(value))),*].into_iter().flatten()
                }

                #(
                #[doc = #is_variants_docs]
                pub fn #is_variants(&self) -> bool {
                    match self {
                        Self::#ty_names(..) => true,
                        _ => false,
                    }
                }
                )*
            }

            #(
            impl From<#ty_names> for Delta {
                fn from(value: #ty_names) -> Self {
                    Self::#ty_names(value)
                }
            }
            )*
        })
        .as_bytes(),
    )
    .unwrap();

    file.into_inner().unwrap().flush().unwrap()
}
