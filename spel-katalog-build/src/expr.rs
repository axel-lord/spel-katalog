//! Functions generating expressions.

use ::std::sync::LazyLock;

use ::convert_case::Boundary;
use ::regex::Regex;
use ::syn::parse_quote;

use crate::string::title_case;

/// Regex to find formating expressions in strings.
pub static FORMAT_RE: LazyLock<Regex> = LazyLock::new(|| {
    let mid = r"\{\w*}";
    let s = r"[^{]";
    let e = r"[^}]";
    Regex::new(&format!("{s}{mid}{e}|{s}{mid}$|^{mid}{e}|^{mid}$")).unwrap()
});

/// Create an expression that returns a static string, which may have been created
/// using a format function.
pub fn str_expr(value: &str) -> ::syn::Expr {
    if FORMAT_RE.is_match(value) {
        parse_quote! {{
            static _LAZY: crate::lazy::Lazy = crate::lazy::Lazy::new(|| format!(#value));
            &_LAZY
        }}
    } else {
        parse_quote!(#value)
    }
}

/// Create a static string expression from either the provided `title` otherwise
/// (should it be missing), by splitting the name.
pub fn title_expr(name: &str, title: Option<&str>) -> ::syn::Expr {
    match title {
        Some(title) => str_expr(title),
        None => {
            let title = ::convert_case::split(&name, &Boundary::defaults())
                .into_iter()
                .map(title_case)
                .collect::<Vec<_>>()
                .join(" ");

            parse_quote!(#title)
        }
    }
}

