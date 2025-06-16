//! Functions and types for parsing strings with variable interpolation.

use ::std::{borrow::Cow, fmt::Display};

use ::nom::{
    Err::Failure,
    Finish, Parser,
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::char,
    combinator::{cut, map, map_parser, peek, value},
    error::{ErrorKind, ParseError},
    multi::many,
    sequence::delimited,
};
use ::tinyvec::TinyVec;

/// Output component for parser separating normal string output and variables
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub enum Component<'input> {
    /// Passthrough string data.
    Slice(&'input str),
    /// Variable to interpolate.
    Variable(&'input str),
}

impl Default for Component<'_> {
    fn default() -> Self {
        Self::Slice("")
    }
}

/// Error returned when parsing fails.
#[derive(Debug, PartialEq, Eq)]
pub struct FmtParseError<'a> {
    err: ::nom::error::Error<&'a str>,
}

impl Display for FmtParseError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.err {
            ::nom::error::Error {
                input: "}",
                code: ErrorKind::Char,
            } => write!(f, "expectded '{{' got '}}'"),
            ::nom::error::Error {
                code: ErrorKind::TakeUntil,
                ..
            } => write!(f, "expected '}}'"),

            other => write!(f, "{other}"),
        }
    }
}

impl ::core::error::Error for FmtParseError<'_> {}

impl<'a> ParseError<&'a str> for FmtParseError<'a> {
    fn from_error_kind(input: &'a str, kind: ErrorKind) -> Self {
        Self {
            err: ::nom::error::Error::from_error_kind(input, kind),
        }
    }

    fn append(input: &'a str, kind: ErrorKind, _other: Self) -> Self {
        Self::from_error_kind(input, kind)
    }
}

/// Error returned when variable interpolation fails.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InterpolationError<'a> {
    /// Failure occurred when parsing input.
    #[error(transparent)]
    Parse(FmtParseError<'a>),
    /// Failure occurred trying to get a variable.
    #[error("could not get variable '{}'", var)]
    Missing {
        /// Variable that did not exist.
        var: &'a str,
    },
}

impl<'a> From<FmtParseError<'a>> for InterpolationError<'a> {
    fn from(value: FmtParseError<'a>) -> Self {
        Self::Parse(value)
    }
}

/// Parse a string with interpolated variables.
pub fn parse_interpolation_components<'a>(
    input: &'a str,
) -> Result<TinyVec<[Component<'a>; 3]>, FmtParseError<'a>> {
    use Component::{Slice, Variable};
    let failure = |input| {
        Err(Failure(FmtParseError::from_error_kind(
            input,
            ErrorKind::Char,
        )))
    };
    many(
        0..,
        alt((
            value(Slice("{"), tag("{{")),
            value(Slice("}"), tag("}}")),
            map_parser(tag("}"), failure),
            map(is_not("{}"), Slice),
            map(
                delimited(
                    char('{'),
                    alt((value("", peek(char('}'))), cut(is_not("{}")))),
                    cut(char('}')),
                ),
                Variable,
            ),
        )),
    )
    .parse(input)
    .finish()
    .map(|(_, comps)| comps)
}

/// Interpolate variables in a string.
pub fn interpolate_str<'a>(
    input: &'a str,
    mut get_var: impl for<'i> FnMut(&'i str) -> Option<&'a str>,
) -> Result<Cow<'a, str>, InterpolationError<'a>> {
    let parsed = parse_interpolation_components(input).map_err(InterpolationError::Parse)?;

    match parsed.as_slice() {
        [Component::Slice(slice)] => Ok(Cow::Borrowed(slice)),
        [Component::Variable(var)] => get_var(var)
            .map(Cow::Borrowed)
            .ok_or_else(|| InterpolationError::Missing { var }),
        components => {
            let mut builder = String::new();

            for component in components {
                match component {
                    Component::Slice(slice) => {
                        builder.push_str(slice);
                    }
                    Component::Variable(var) => builder
                        .push_str(get_var(var).ok_or_else(|| InterpolationError::Missing { var })?),
                }
            }

            Ok(Cow::Owned(builder))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(missing_docs)]

    use ::std::{collections::HashMap, sync::LazyLock};

    use ::nom::error::ErrorKind;

    use super::*;

    fn get(key: &str) -> Option<&'static str> {
        static MAP: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
            HashMap::from_iter([
                ("a", "first"),
                ("b", "second"),
                ("c", "third"),
                ("LBRACE", "{"),
                ("RBRACE", "}"),
            ])
        });
        Some(MAP.get(key)?)
    }

    fn n_err<'a, T>(input: &'a str, code: ErrorKind) -> Result<T, FmtParseError<'a>> {
        Err(FmtParseError {
            err: ::nom::error::Error { input, code },
        })
    }

    fn new_missing<'a>(var: &'a str) -> InterpolationError<'a> {
        InterpolationError::Missing { var }
    }

    #[test]
    fn parse_empty() {
        assert_eq!(
            parse_interpolation_components("{}"),
            Ok([Component::Variable("")].into_iter().collect())
        );
        assert_eq!(
            parse_interpolation_components("{{}"),
            n_err("}", ErrorKind::Char)
        );
        assert_eq!(parse_interpolation_components(""), Ok(TinyVec::new()));
        assert_eq!(
            parse_interpolation_components("{}}"),
            n_err("}", ErrorKind::Char)
        );
        assert_eq!(
            parse_interpolation_components("{ababab"),
            n_err("", ErrorKind::Char)
        );
    }

    #[test]
    fn test_interpolate_none() {
        let result = interpolate_str("hello there", get);
        assert_eq!(result, Ok(Cow::Borrowed("hello there")));
        assert!(matches!(result, Ok(Cow::Borrowed(..))));
    }

    #[test]
    fn test_interpolate_only() {
        let result = interpolate_str("{a}", get);
        assert_eq!(result, Ok(Cow::Borrowed("first")));
        assert!(matches!(result, Ok(Cow::Borrowed(..))));
    }

    #[test]
    fn test_interpolate_multiple() {
        let result = interpolate_str("{LBRACE}{a}, {b}, {c}{RBRACE}", get).unwrap();
        assert_eq!(result.as_ref(), "{first, second, third}");
        assert!(matches!(result, Cow::Owned(..)));
    }

    #[test]
    fn test_interpolate_missing() {
        assert_eq!(interpolate_str("{d}, {e}", get), Err(new_missing("d")));

        assert_eq!(
            interpolate_str("{{}", get),
            n_err("}", ErrorKind::Char).map_err(From::from)
        );

        assert_eq!(interpolate_str("{}", get), Err(new_missing("")));

        let result = interpolate_str("{}}", get);
        assert_eq!(result, n_err("}", ErrorKind::Char).map_err(From::from));

        let result = interpolate_str("{{}}", get);
        assert_eq!(result, Ok(Cow::Owned(String::from("{}"))));
    }
}
