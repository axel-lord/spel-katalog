//! Functions and types for parsing strings with variable interpolation.

use ::std::{borrow::Cow, fmt::Display};

use ::nom::{
    Finish, Parser,
    branch::alt,
    bytes::complete::{is_not, tag, take_until},
    character::complete::char,
    combinator::{cut, eof, map_parser},
    error::ErrorKind,
    multi::{many, many_till},
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
pub struct ParseError<'a> {
    err: ::nom::error::Error<&'a str>,
}

impl Display for ParseError<'_> {
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

impl ::core::error::Error for ParseError<'_> {}

/// Error returned when variable interpolation fails.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InterpolationError<'a> {
    /// Failure occurred when parsing input.
    #[error(transparent)]
    Parse(ParseError<'a>),
    /// Failure occurred trying to get a variable.
    #[error("could not get variable '{}'", var)]
    Missing {
        /// Variable that did not exist.
        var: &'a str,
    },
}

/// Parse a string with interpolated variables.
pub fn parse_interpolation_components_tv<'a>(
    input: &'a str,
) -> Result<TinyVec<[Component<'a>; 3]>, ParseError<'a>> {
    many(
        0..,
        alt((
            map_parser(tag("}"), |input| {
                Err(::nom::Err::Failure(::nom::error::Error {
                    input,
                    code: ErrorKind::Char,
                }))
            }),
            is_not("{}").map(Component::Slice),
            delimited(char('{'), cut(take_until("}")), char('}')).map(Component::Variable),
        )),
    )
    .parse(input)
    .finish()
    .map_err(|err| ParseError { err })
    .map(|(_, comps)| comps)
}

/// Parse a string with interpolated variables.
pub fn parse_interpolation_components<'a>(
    input: &'a str,
) -> Result<Vec<Component<'a>>, ParseError<'a>> {
    many_till(
        alt((
            is_not("{}").map(Component::Slice),
            delimited(char('{'), take_until("}"), char('}')).map(Component::Variable),
        )),
        eof,
    )
    .parse(input)
    .finish()
    .map_err(|err| ParseError { err })
    .map(|(_, (components, _))| components)
}

/// Interpolate variables in a string.
pub fn interpolate_str<'a>(
    input: &'a str,
    mut get_var: impl for<'i> FnMut(&'i str) -> Option<&'a str>,
) -> Result<Cow<'a, str>, InterpolationError<'a>> {
    let parsed = parse_interpolation_components_tv(input).map_err(InterpolationError::Parse)?;

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

    fn n_err<'a, T>(input: &'a str, code: ErrorKind) -> Result<T, ParseError<'a>> {
        Err(ParseError {
            err: ::nom::error::Error { input, code },
        })
    }

    #[test]
    fn parse_empty() {
        parse_interpolation_components("{}").unwrap();
        parse_interpolation_components("{{}").unwrap();
        assert_eq!(parse_interpolation_components(""), Ok(Vec::new()));
        assert_eq!(
            parse_interpolation_components("{}}"),
            n_err("}", ErrorKind::Char)
        );
        assert_eq!(
            parse_interpolation_components("{ababab"),
            n_err("ababab", ErrorKind::TakeUntil)
        );
    }

    #[test]
    fn test_interpolate_none() {
        let result = interpolate_str("hello there", get).unwrap();
        assert_eq!(result.as_ref(), "hello there",);
        assert!(matches!(result, Cow::Borrowed(..)));
    }

    #[test]
    fn test_interpolate_only() {
        let result = interpolate_str("{a}", get).unwrap();
        assert_eq!(result.as_ref(), "first");
        assert!(matches!(result, Cow::Borrowed(..)));
    }

    #[test]
    fn test_interpolate_multiple() {
        let result = interpolate_str("{LBRACE}{a}, {b}, {c}{RBRACE}", get).unwrap();
        assert_eq!(result.as_ref(), "{first, second, third}");
        assert!(matches!(result, Cow::Owned(..)));
    }

    #[test]
    fn test_interpolate_missing() {
        let result = interpolate_str("{d}, {e}", get);
        assert_eq!(result, Err(InterpolationError::Missing { var: "d" }));

        let result = interpolate_str("{{}", get);
        assert_eq!(result, Err(InterpolationError::Missing { var: "{" }));

        let result = interpolate_str("{}", get);
        assert_eq!(result, Err(InterpolationError::Missing { var: "" }));

        let result = interpolate_str("{}}", get);
        assert_eq!(
            result,
            Err(InterpolationError::Parse(ParseError {
                err: ::nom::error::Error {
                    input: "}",
                    code: ::nom::error::ErrorKind::Char,
                }
            }))
        );

        let result = interpolate_str("{{}}", get);
        assert_eq!(
            result,
            Err(InterpolationError::Parse(ParseError {
                err: ::nom::error::Error {
                    input: "}",
                    code: ::nom::error::ErrorKind::Char,
                }
            }))
        );
    }
}
