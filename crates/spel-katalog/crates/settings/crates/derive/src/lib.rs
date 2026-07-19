//! Proc macros for settings.

use ::proc_macro::TokenStream;
use ::spel_katalog_settings_derive_lib as lib;

/// Derive the `Help` trait.
///
/// ```
/// #[derive(Help)]
/// #[help = "A number"]
/// #[settings(help = "A number")]
/// struct Number(i32);
/// ```
#[proc_macro_derive(Help, attributes(help, settings))]
pub fn derive_help(tokens: TokenStream) -> TokenStream {
    lib::derive_help(tokens.into()).into()
}

/// Derive the `Title` trait.
///
/// ```
/// #[derive(Title)]
/// #[title = "Number"]
/// #[settings(title = "Number")]
/// struct Number(i32);
/// ```
#[proc_macro_derive(Title, attributes(title, settings))]
pub fn derive_title(tokens: TokenStream) -> TokenStream {
    lib::derive_title(tokens.into()).into()
}

/// Derive the `DefaultStr` trait.
///
/// ```
/// #[derive(DefaultStr)]
/// #[default_str = "23"]
/// #[settings(default_str = "23")]
/// struct Number(i32);
/// ```
#[proc_macro_derive(DefaultStr, attributes(default_str, settings))]
pub fn derive_default_str(tokens: TokenStream) -> TokenStream {
    lib::derive_default_str(tokens.into()).into()
}

/// Derive `TrustedVariants` trait.
#[proc_macro_derive(TrustedVariants, attributes(variants, settings))]
pub fn derive_variants(tokens: TokenStream) -> TokenStream {
    lib::derive_variants(tokens.into()).into()
}
