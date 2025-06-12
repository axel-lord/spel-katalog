//! Functions and types for parsing strings with variable interpolation.

use ::std::borrow::Cow;

use ::nom::{
    Finish, IResult, Parser,
    bytes::{complete::take_till, tag},
    sequence::delimited,
};

/// Output component for parser separating normal string output and variables
#[derive(Debug)]
pub enum Component<'input> {
    /// Passthrough string data.
    Slice(&'input str),
    /// Variable to interpolate.
    Variable(&'input str),
}

/// Error returned when parsing fails.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("{}", err)]
pub struct ParseError<'a> {
    err: ::nom::error::Error<&'a str>,
}

/// Error returned when variable interpolation fails.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InterpolationError<'a> {
    /// Failure occurred when parsing input.
    #[error(transparent)]
    Parse(ParseError<'a>),
    /// Failure occurred trying to get a variable.
    #[error("could not get variable '{}'", var)]
    Get {
        /// Variable that did not exist.
        var: &'a str,
    },
}

fn parse_interpolate_inner<'a>(mut input: &'a str) -> IResult<&'a str, Vec<Component<'a>>> {
    let mut components = Vec::new();
    loop {
        let (remainder, content) = take_till(|c| c == '{')(input)?;

        if !content.is_empty() {
            components.push(Component::Slice(content));
        }

        if remainder.is_empty() {
            return Ok(("", components));
        }

        let (remainder, var) =
            delimited(tag("{"), take_till(|c| c == '}'), tag("}")).parse(remainder)?;
        components.push(Component::Variable(var));

        input = remainder;
    }
}

/// Parse a string with interpolated variables.
pub fn parse_interpolation_components<'a>(
    input: &'a str,
) -> Result<Vec<Component<'a>>, ParseError<'a>> {
    parse_interpolate_inner(input)
        .finish()
        .map(|(_, components)| components)
        .map_err(|err| ParseError { err })
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
            .ok_or_else(|| InterpolationError::Get { var }),
        components => {
            let mut builder = String::new();

            for component in components {
                match component {
                    Component::Slice(slice) => {
                        builder.push_str(slice);
                    }
                    Component::Variable(var) => builder
                        .push_str(get_var(var).ok_or_else(|| InterpolationError::Get { var })?),
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
        assert_eq!(result, Err(InterpolationError::Get { var: "d" }));

        let result = interpolate_str("{{}", get);
        assert_eq!(result, Err(InterpolationError::Get { var: "{" }));

        let result = interpolate_str("{}", get);
        assert_eq!(result, Err(InterpolationError::Get { var: "" }));

        let result = interpolate_str("{}}", get);
        assert_eq!(result, Err(InterpolationError::Get { var: "" }));

        let result = interpolate_str("{{}}", get);
        assert_eq!(result, Err(InterpolationError::Get { var: "{" }));
    }
}
