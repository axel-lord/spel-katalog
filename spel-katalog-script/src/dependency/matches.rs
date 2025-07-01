//! Dependency checking for value match.

use ::regex::RegexBuilder;
use ::serde::{Deserialize, Serialize};

use crate::{
    dependency::{DependencyError, DependencyResult, failure::failure},
    maybe_single::MaybeSingle,
    string_visitor::VisitStrings,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Inner {
    Match {
        /// Values to check for.
        #[serde(alias = "value")]
        values: MaybeSingle,

        /// Pattern to match.
        #[serde(alias = "matches")]
        r#match: String,
    },
    IMatch {
        /// Values to check for.
        #[serde(alias = "value")]
        values: MaybeSingle,

        /// Pattern to match.
        #[serde(alias = "imatches")]
        imatch: String,
    },
}

/// Check if values mayches a pattern.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(from = "Inner", into = "Inner")]
pub struct Matches {
    /// Values to check for.
    values: MaybeSingle,

    /// Pattern to match.
    pattern: String,

    /// Use case insensitive matching.
    insensitive: bool,
}

impl Matches {
    pub async fn check(&self, panic: bool) -> Result<DependencyResult, DependencyError> {
        let Self {
            values,
            pattern,
            insensitive,
        } = self;
        let re = RegexBuilder::new(&pattern)
            .case_insensitive(*insensitive)
            .build()?;

        for value in values.as_slice() {
            if !re.is_match(value) {
                return Ok(failure!(
                    panic,
                    "pattern /{}/ did not match {:?}",
                    re.as_str(),
                    value
                ));
            }
        }
        Ok(DependencyResult::Success)
    }

    /// Visit all parsed string values.
    pub fn visit_strings<E>(&mut self, v: &mut dyn VisitStrings<E>) -> Result<(), E> {
        let Self {
            values,
            pattern,
            insensitive: _,
        } = self;

        v.visit_slice(values.as_mut_slice())?
            .visit(pattern)?
            .finish()
    }
}

impl From<Inner> for Matches {
    fn from(value: Inner) -> Self {
        match value {
            Inner::Match { values, r#match } => Self {
                values,
                pattern: r#match,
                insensitive: false,
            },
            Inner::IMatch { values, imatch } => Self {
                values,
                pattern: imatch,
                insensitive: true,
            },
        }
    }
}

impl From<Matches> for Inner {
    fn from(value: Matches) -> Self {
        let Matches {
            values,
            pattern,
            insensitive,
        } = value;
        if insensitive {
            Self::IMatch {
                values,
                imatch: pattern,
            }
        } else {
            Self::Match {
                values,
                r#match: pattern,
            }
        }
    }
}
