use ::std::{
    borrow::Cow,
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::LazyLock,
};

use ::convert_case::{Case, Casing};
use ::quote::format_ident;
use ::regex::Regex;
use ::rustc_hash::FxHashMap;
use ::serde::Deserialize;
use ::syn::parse_quote;

static FORMAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    let mid = r"\{\w*\}";
    let s = r"[^{]";
    let e = r"[^}]";
    Regex::new(&format!("{s}{mid}{e}|{s}{mid}$|^{mid}{e}|^{mid}$")).unwrap()
});

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase())
        .into_iter()
        .flatten()
        .chain(chars)
        .collect()
}

fn str_expr(value: &str) -> ::syn::Expr {
    if FORMAT_RE.is_match(value) {
        parse_quote! {{
            static _LAZY: crate::lazy::Lazy = crate::lazy::Lazy::new(|| format!(#value));
            &_LAZY
        }}
    } else {
        parse_quote!(#value)
    }
}

fn doc_str(value: &str) -> Cow<str> {
    if value.ends_with('.') {
        Cow::Borrowed(value)
    } else {
        Cow::Owned(format!("{value}."))
    }
}

fn title_body(name: &str, title: Option<&str>) -> ::syn::Expr {
    match title {
        Some(title) => str_expr(title),
        None => {
            let title = ::convert_case::split(&name, &::convert_case::Boundary::defaults())
                .into_iter()
                .map(title_case)
                .collect::<Vec<_>>()
                .join(" ");

            parse_quote!(#title)
        }
    }
}

struct Emit {
    ty: ::syn::Item,
    impls: ::syn::File,
    from_str: ::syn::File,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
enum Setting {
    Enum {
        title: Option<String>,
        help: String,
        variants: Vec<String>,
        default: String,
    },
    Path {
        title: Option<String>,
        help: String,
        path: String,
    },
}

impl Setting {
    /*
    fn title(&self) -> Option<&str> {
        match self {
            Setting::Enum { title, .. } | Setting::Path { title, .. } => title.as_deref(),
        }
    }
    */
    fn help(&self) -> &str {
        match self {
            Setting::Enum { help, .. } | Setting::Path { help, .. } => help,
        }
    }
    fn emit_type(&self, name: &str) -> Emit {
        let ident = format_ident!("{}", name.to_case(Case::Pascal));
        match self {
            Setting::Enum {
                title,
                help,
                variants,
                default,
            } => {
                let doc = doc_str(&help);
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
                let title_body = title_body(name, title.as_deref());

                let parse_err_ident = format_ident!("Parse{ident}Error");
                let parse_err_doc = format!(
                    "Error used by [FromStr][::core::str::FromStr] implementation for [{name}]",
                    name = name.to_case(Case::Pascal)
                );
                let parse_err_msg = format!("string not one of {}", variants.join(", "));

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
                let impls = parse_quote! {
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

                    impl crate::settings::Title for #ident {
                        fn title() -> &'static str {
                            #title_body
                        }
                    }

                    impl crate::settings::Variants for #ident {
                        const VARIANTS: &[Self] = &[#( Self::#variant_idents ),*];
                    }

                    impl crate::settings::DefaultStr for #ident {
                        fn default_str() -> &'static str {
                            #default
                        }
                    }


                    impl ::core::default::Default for #ident {
                        fn default() -> Self {
                            Self :: #default_ident
                        }
                    }

                    impl ::core::fmt::Display for #ident {
                        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                            f.write_str(self.as_str())
                        }
                    }

                    impl ::core::convert::AsRef<str> for #ident {
                        fn as_ref(&self) -> &str {
                            self.as_str()
                        }
                    }

                    impl ::core::convert::From<#ident> for ::std::string::String {
                        fn from(value: #ident) -> Self {
                            value.as_str().into()
                        }
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
            Setting::Path { title, help, path } => {
                let title_body = title_body(name, title.as_deref());
                let doc = doc_str(&help);
                let default_value = str_expr(&path);

                let ty = parse_quote! {
                    #[derive(
                        Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash,
                        ::serde::Serialize, ::serde::Deserialize
                    )]
                    #[doc = #doc]
                    #[serde(transparent)]
                    pub struct #ident(String);
                };

                let impls = parse_quote! {
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

                    impl crate::settings::Title for #ident {
                        fn title() -> &'static str {
                            #title_body
                        }
                    }

                    impl crate::settings::DefaultStr for #ident {
                        fn default_str() -> &'static str {
                            #default_value
                        }
                    }

                    impl ::core::default::Default for #ident {
                        fn default() -> Self {
                            Self(<Self as crate::settings::DefaultStr>::default_str().into())
                        }
                    }

                    impl ::core::fmt::Display for #ident {
                        fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                            f.write_str(self.as_str())
                        }
                    }

                    impl ::core::convert::AsRef<str> for #ident {
                        fn as_ref(&self) -> &str {
                            self.as_str()
                        }
                    }

                    impl ::core::convert::AsRef<::std::path::Path> for #ident {
                        fn as_ref(&self) -> &::std::path::Path {
                            self.as_path()
                        }
                    }

                    impl ::core::convert::AsRef<::std::ffi::OsStr> for #ident {
                        fn as_ref(&self) -> &::std::ffi::OsStr {
                            self.as_os_str()
                        }
                    }

                    impl ::core::convert::From<#ident> for ::std::string::String {
                        fn from(value: #ident) -> Self {
                            value.into_inner()
                        }
                    }

                    impl ::core::convert::From<::std::string::String> for #ident{
                        fn from(value: ::std::string::String) -> Self {
                            Self::new(value)
                        }
                    }

                    impl ::std::ops::Deref for #ident {
                        type Target = str;
                        fn deref(&self) -> &Self::Target {
                            self.as_str()
                        }
                    }
                };

                let from_str = parse_quote! {};

                Emit {
                    ty,
                    impls,
                    from_str,
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct Settings {
    #[serde(flatten)]
    settings: FxHashMap<String, Setting>,
}

impl Settings {
    fn write(self, out_dir: &Path) {
        let mut file = BufWriter::new(File::create(out_dir.join("settings.rs")).unwrap());
        let Self { settings } = self;

        let mut emitted = settings
            .iter()
            .map(|(name, setting)| (name, setting, setting.emit_type(name)))
            .collect::<Vec<_>>();
        emitted.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

        let from_str = emitted.iter().map(|(.., e)| &e.from_str);
        let types = emitted.iter().map(|(.., e)| &e.ty);
        let impls = emitted.iter().map(|(.., e)| &e.impls);

        let (ty_names, ty_doc, is_variants, is_variants_docs, field_names) = emitted
            .iter()
            .map(|(n, s, ..)| {
                let pascal_ident = n.to_case(Case::Pascal);
                let snake_ident = n.to_case(Case::Snake);
                (
                    format_ident!("{pascal_ident}"),
                    doc_str(s.help()),
                    format_ident!("is_{snake_ident}",),
                    format!("Check if delta is of the `{pascal_ident}` variant.",),
                    format_ident!("{snake_ident}"),
                )
            })
            .collect::<(Vec<_>, Vec<_>, Vec<_>, Vec<_>, Vec<_>)>();

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
                    #field_names: Option<#ty_names>,
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
}

fn main() {
    println!("cargo::rerun-if-changed=src/settings.toml");
    let out_dir = ::std::env::var("OUT_DIR").unwrap();

    ::toml::from_str::<Settings>(&::std::fs::read_to_string("src/settings.toml").unwrap())
        .unwrap()
        .write(out_dir.as_ref());
}
