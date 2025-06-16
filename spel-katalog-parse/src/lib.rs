//! Functions and types for parsing strings with variable interpolation.

use ::std::{
    borrow::Cow,
    fmt::{Debug, Display},
};

use ::derive_more::IsVariant;
use ::nom::{
    Compare, FindToken, Finish, IResult, Input, Parser,
    branch::alt,
    bytes::complete::{is_not, tag, take},
    combinator::{cut, map, map_parser, peek},
    error::{ErrorKind, ParseError},
    multi::many,
    sequence::delimited,
};
use ::nom_locate::LocatedSpan;
use ::smallvec::SmallVec;

/// Output component for parser separating normal string output and variables
#[derive(Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy, IsVariant)]
pub enum Component<I> {
    /// Passthrough string data.
    Slice(I),
    /// Variable to interpolate.
    Variable(I),
}

impl<I> Display for Component<I>
where
    I: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Component::Slice(slice) => Display::fmt(slice, f),
            Component::Variable(var) => write!(f, "{{{var}}}"),
        }
    }
}

impl<I> Component<I> {
    /// Map component content.
    pub fn map<F: FnOnce(I) -> T, T>(self, f: F) -> Component<T> {
        match self {
            Component::Slice(value) => Component::Slice(f(value)),
            Component::Variable(value) => Component::Variable(f(value)),
        }
    }
}

impl<I> Component<&I>
where
    I: ToOwned + ?Sized,
{
    /// Convert into a `FmtParseError<I::Owned>`.
    pub fn cloned(self) -> Component<I::Owned> {
        match self {
            Component::Slice(s) => Component::Slice(s.to_owned()),
            Component::Variable(v) => Component::Variable(v.to_owned()),
        }
    }
}

impl<I> Default for Component<I>
where
    I: Default,
{
    fn default() -> Self {
        Self::Slice(I::default())
    }
}

/// Error returned when parsing fails.
#[derive(Debug, PartialEq, Eq)]
pub enum FmtParseError<I> {
    /// A forwarded nom parse error.
    ParseError {
        /// Nom error.
        err: ::nom::error::Error<I>,
    },
    /// Input was not allowed at position.
    Blocked(I),
}

impl<I> FmtParseError<I> {
    /// Map input type `I` to `T`.
    pub fn map_input<F, T>(self, f: F) -> FmtParseError<T>
    where
        F: FnOnce(I) -> T,
    {
        match self {
            FmtParseError::ParseError {
                err: ::nom::error::Error { input, code },
            } => FmtParseError::ParseError {
                err: ::nom::error::Error {
                    input: f(input),
                    code,
                },
            },
            FmtParseError::Blocked(i) => FmtParseError::Blocked(f(i)),
        }
    }
}

impl<I: ToOwned + ?Sized> FmtParseError<&I> {
    /// Convert into a `FmtParseError<I::Owned>`.
    pub fn cloned(self) -> FmtParseError<I::Owned> {
        match self {
            FmtParseError::ParseError { err } => FmtParseError::ParseError { err: err.cloned() },
            FmtParseError::Blocked(i) => FmtParseError::Blocked(i.to_owned()),
        }
    }
}

impl<I: ToOwned + ?Sized> FmtParseError<&mut I> {
    /// Convert into an `FmtParseError<I::Owned>`.
    pub fn cloned(self) -> FmtParseError<I::Owned> {
        match self {
            FmtParseError::ParseError { err } => FmtParseError::ParseError { err: err.cloned() },
            FmtParseError::Blocked(i) => FmtParseError::Blocked(i.to_owned()),
        }
    }
}

impl<I> Display for FmtParseError<I>
where
    I: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FmtParseError::ParseError { err } => Display::fmt(err, f),
            FmtParseError::Blocked(i) => write!(f, "not allowed at given position '{i}'"),
        }
    }
}

impl<I> ::core::error::Error for FmtParseError<I> where I: Display + Debug {}

impl<I> ParseError<I> for FmtParseError<I> {
    fn from_error_kind(input: I, kind: ErrorKind) -> Self {
        Self::ParseError {
            err: ::nom::error::Error::from_error_kind(input, kind),
        }
    }

    fn append(input: I, kind: ErrorKind, _other: Self) -> Self {
        Self::from_error_kind(input, kind)
    }
}

/// Error returned when variable interpolation fails.
#[derive(Debug, PartialEq, Eq)]
pub enum InterpolationError<I> {
    /// Failure occurred when parsing input.
    Parse(FmtParseError<I>),
    /// Failure occurred trying to get a variable.
    Missing {
        /// Variable that did not exist.
        var: I,
    },
}

impl<I> InterpolationError<I> {
    /// Map input type `I` to `T`.
    pub fn map_input<F, T>(self, f: F) -> InterpolationError<T>
    where
        F: FnOnce(I) -> T,
    {
        match self {
            InterpolationError::Parse(fmt_parse_error) => {
                InterpolationError::Parse(fmt_parse_error.map_input(f))
            }
            InterpolationError::Missing { var } => InterpolationError::Missing { var: f(var) },
        }
    }
}

impl<I: ToOwned + ?Sized> InterpolationError<&I> {
    /// Convert into an `InterpolationError<I::Owned>`.
    pub fn cloned(self) -> InterpolationError<I::Owned> {
        match self {
            InterpolationError::Parse(fmt_parse_error) => {
                InterpolationError::Parse(fmt_parse_error.cloned())
            }
            InterpolationError::Missing { var } => InterpolationError::Missing {
                var: var.to_owned(),
            },
        }
    }
}

impl<I: ToOwned + ?Sized> InterpolationError<&mut I> {
    /// Convert into an `InterpolationError<I::Owned>`.
    pub fn cloned(self) -> InterpolationError<I::Owned> {
        match self {
            InterpolationError::Parse(fmt_parse_error) => {
                InterpolationError::Parse(fmt_parse_error.cloned())
            }
            InterpolationError::Missing { var } => InterpolationError::Missing {
                var: var.to_owned(),
            },
        }
    }
}

impl<I> ::core::error::Error for InterpolationError<I> where I: Display + Debug {}

impl<I> Display for InterpolationError<I>
where
    I: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpolationError::Parse(fmt_parse_error) => Display::fmt(fmt_parse_error, f),
            InterpolationError::Missing { var } => write!(f, "could not get variable '{var}'"),
        }
    }
}

impl<I> From<FmtParseError<I>> for InterpolationError<I> {
    fn from(value: FmtParseError<I>) -> Self {
        Self::Parse(value)
    }
}

fn variable<I>(input: I) -> IResult<I, Component<I>, FmtParseError<I>>
where
    I: Input + for<'a> Compare<&'a str>,
    for<'a> &'a str: FindToken<<I as Input>::Item>,
{
    use Component::Variable;
    map(
        delimited(
            tag("{"),
            alt((map_parser(peek(tag("}")), take(0usize)), cut(is_not("{}")))),
            cut(tag("}")),
        ),
        Variable,
    )
    .parse(input)
}

fn escape<T, I>(seq: T) -> impl Parser<I, Output = Component<I>, Error = FmtParseError<I>>
where
    T: Input,
    I: Input + Compare<T>,
{
    map(map_parser(tag(seq), take(1usize)), Component::Slice)
}

fn block<T, I, O>(seq: T) -> impl Parser<I, Output = O, Error = FmtParseError<I>>
where
    T: Input,
    I: Input + Compare<T>,
{
    use ::nom::Err::Failure;
    map_parser(tag(seq), |i| Err(Failure(FmtParseError::Blocked(i))))
}

/// Parse a string with interpolated variables.
pub fn parse_interpolation_components<I, C>(input: I) -> Result<C, FmtParseError<I>>
where
    I: Clone + Input + for<'a> Compare<&'a str>,
    C: Extend<Component<I>> + Default,
    for<'a> &'a str: FindToken<<I as Input>::Item>,
{
    use Component::Slice;
    many(
        0..,
        alt((
            escape("{{"),
            escape("}}"),
            block("}"),
            map(is_not("{}"), Slice),
            variable,
        )),
    )
    .parse(input)
    .finish()
    .map(|(_, comps)| comps)
}

/// Interpolate variables in a string with more lax allocation.
pub fn interpolate_string<F>(
    input: &str,
    mut get_var: F,
) -> Result<String, InterpolationError<String>>
where
    F: for<'k> FnMut(&'k str) -> Option<String>,
{
    let parsed = parse_interpolation_components::<_, SmallVec<[_; 3]>>(input)
        .map_err(|err| InterpolationError::Parse(err.cloned()))?;
    let mut builder = String::new();

    for component in parsed {
        match component {
            Component::Slice(slc) => builder.push_str(slc),
            Component::Variable(var) => {
                builder.push_str(&get_var(var).ok_or_else(|| InterpolationError::Missing {
                    var: var.to_owned(),
                })?)
            }
        }
    }

    Ok(builder)
}

/// Interpolate variables in a string.
pub fn interpolate_str<'i, F>(
    input: &'i str,
    mut get_var: F,
) -> Result<Cow<'i, str>, InterpolationError<LocatedSpan<&'i str>>>
where
    F: for<'k> FnMut(&'k str) -> Option<&'i str>,
{
    let parsed = parse_interpolation_components::<_, SmallVec<[_; 3]>>(LocatedSpan::new(input))?;

    match parsed.as_slice() {
        [Component::Slice(slice)] => Ok(Cow::Borrowed(slice)),
        [Component::Variable(var)] => get_var(var)
            .map(Cow::Borrowed)
            .ok_or_else(|| InterpolationError::Missing { var: *var }),
        components => {
            let mut builder = String::new();

            for component in components {
                match component {
                    Component::Slice(slice) => {
                        builder.push_str(slice);
                    }
                    Component::Variable(var) => builder.push_str(
                        get_var(var).ok_or_else(|| InterpolationError::Missing { var: *var })?,
                    ),
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

    fn n_err<'a, T>(input: &'a str, code: ErrorKind) -> Result<T, FmtParseError<&'a str>> {
        Err(FmtParseError::from_error_kind(input, code))
    }

    fn missing<'a, T>(var: &'a str) -> Result<T, InterpolationError<&'a str>> {
        Err(InterpolationError::Missing { var })
    }

    fn blocked<'a, T>(input: &'a str) -> Result<T, FmtParseError<&'a str>> {
        Err(FmtParseError::Blocked(input))
    }

    #[test]
    fn parse_empty() {
        assert_eq!(
            parse_interpolation_components::<_, Vec<_>>("{}"),
            Ok([Component::Variable("")].into_iter().collect())
        );
        assert_eq!(
            parse_interpolation_components::<_, Vec<_>>("{{}"),
            blocked("}")
        );
        assert_eq!(
            parse_interpolation_components::<_, Vec<_>>(""),
            Ok(Vec::new())
        );
        assert_eq!(
            parse_interpolation_components::<_, Vec<_>>("{}}"),
            blocked("}")
        );
        assert_eq!(
            parse_interpolation_components::<_, Vec<_>>("{ababab"),
            n_err("", ErrorKind::Tag)
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
        assert_eq!(
            interpolate_str("{d}, {e}", get).map_err(|err| err.map_input(|i| i.into_fragment())),
            missing("d")
        );

        assert_eq!(
            interpolate_str("{{}", get).map_err(|err| err.map_input(|i| i.into_fragment())),
            blocked("}").map_err(From::from)
        );

        assert_eq!(
            interpolate_str("{}", get).map_err(|err| err.map_input(|i| i.into_fragment())),
            missing("")
        );

        assert_eq!(
            interpolate_str("{}}", get).map_err(|err| err.map_input(|i| i.into_fragment())),
            blocked("}").map_err(From::from)
        );

        assert_eq!(
            interpolate_str("{{}}", get).map_err(|err| err.map_input(|i| i.into_fragment())),
            Ok(Cow::Owned(String::from("{}")))
        );
    }
}
