//! Script deps.

use ::std::ops::Not;

use ::bon::Builder;
use ::derive_more::{From, IsVariant};
use ::regex::Regex;
use ::serde::{Deserialize, Serialize};
use ::tap::Tap;

use crate::{
    environment::Env,
    exec::{Exec, ExecError},
    maybe_single::MaybeSingle,
};

/// Error type returned by dependency check.
#[derive(Debug, ::thiserror::Error)]
pub enum DependencyError {
    /// Running an executable failed.
    #[error(transparent)]
    RunExec(#[from] ExecError),
    /// A Script dependency was not ran before this one.
    #[error("no result available for {0:?}")]
    MissingDep(String),
    /// Runtime for async operations could not be set up.
    #[error("could not setup runtime, {0}")]
    Runtime(#[source] ::std::io::Error),
    /// Regex pattern could not be compiled
    #[error(transparent)]
    ReCompilation(#[from] ::regex::Error),
}

/// The result of a dependency check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, IsVariant)]
pub enum DependencyResult {
    /// Failure which should stop all scripts.
    Failure = 2,
    /// Failure which should only stop current script.
    TryFailure = 1,
    /// Success, continue on.
    #[default]
    Success = 0,
}

impl FromIterator<DependencyResult> for DependencyResult {
    fn from_iter<T: IntoIterator<Item = DependencyResult>>(iter: T) -> Self {
        iter.into_iter().max().unwrap_or_default()
    }
}

/// A script dependency, either before any script is run, `require`,
/// or before this script is run `assert`.
///
/// `assert` script dependencies error at the end of the `require` check
/// if the script does not exist/will not run. They are then ran again
/// at assert step.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Builder)]
pub struct Dependency {
    /// Kind of dependency.
    #[serde(flatten)]
    #[builder(into)]
    pub kind: DependencyKind,

    /// Failure will stop current script but no others unless current one is required.
    #[serde(default, skip_serializing_if = "Not::not")]
    #[builder(default)]
    pub panic: bool,
}

/// Different kinds of dependency.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, From, IsVariant)]
#[serde(untagged)]
pub enum DependencyKind {
    /// Depend on the existance/success of a scripts dependencies.
    #[from(ignore)]
    Script {
        /// Id of dependency.
        #[serde(rename = "script")]
        id: String,
    },
    /// Depend on the success of an executable.
    Exec(Exec),
    /// Depend on a group of values being the same.
    Equals {
        /// Values to check if equal.
        #[serde(rename = "equals", alias = "equal")]
        values: Vec<String>,
    },
    /// Depend on a value being in a list.
    In {
        /// Values to check for.
        #[serde(alias = "value")]
        values: MaybeSingle,

        /// Collection to check for values in.
        #[serde(rename = "in")]
        collection: Vec<String>,
    },
    /// Check if values mayches a pattern.
    Matches {
        /// Values to check for.
        #[serde(alias = "value")]
        values: MaybeSingle,

        /// Pattern to match.
        #[serde(rename = "match", alias = "matches")]
        pattern: String,
    },
}

impl DependencyKind {
    /// Create a script dependency on the given id.
    pub fn script(id: impl Into<String>) -> Self {
        Self::Script { id: id.into() }
    }

    /// Create an executable dependency.
    pub fn exec(exec: impl Into<Exec>) -> Self {
        Self::Exec(exec.into())
    }
}

impl Dependency {
    /// Check success of dependency.
    pub async fn check(
        &self,
        env: &Env,
        get_prior: impl for<'k> FnOnce(&'k str) -> Option<DependencyResult>,
    ) -> Result<DependencyResult, DependencyError> {
        let Self { kind, panic } = self;
        let failure = || {
            if *panic {
                DependencyResult::Failure
            } else {
                DependencyResult::TryFailure
            }
        };
        let level = if *panic {
            ::log::Level::Error
        } else {
            ::log::Level::Info
        };

        let result = match kind {
            DependencyKind::Script { id } => {
                let Some(prior) = get_prior(&id) else {
                    return Err(DependencyError::MissingDep(id.clone()));
                };

                if prior.is_success() {
                    prior
                } else {
                    ::log::log!(level, "script dependency did not succeed, {id}");
                    failure()
                }
            }
            DependencyKind::Exec(exec) => {
                let status = exec.run(env).await?;

                if status.success() {
                    DependencyResult::Success
                } else {
                    ::log::log!(level, "dependency exec failed, {status}");
                    failure()
                }
            }
            DependencyKind::Equals { values } => match values.as_slice() {
                [] => DependencyResult::Success,
                [head, remainder @ ..] if remainder.iter().all(|e| e == head) => {
                    DependencyResult::Success
                }
                values => {
                    ::log::log!(level, "equality check failed, values:\n{values:#?}");
                    failure()
                }
            },
            DependencyKind::In { values, collection } => {
                let values = values.clone().dedup();
                let collection = collection.clone().tap_mut(|collection| {
                    collection.sort_unstable();
                    collection.dedup();
                });

                'blk: {
                    for value in values.as_slice() {
                        if collection.binary_search(&value).is_err() {
                            ::log::log!(level, "value {value} not in collection\n{collection:#?}");
                            break 'blk failure();
                        }
                    }
                    DependencyResult::Success
                }
            }
            DependencyKind::Matches { values, pattern } => {
                let re = Regex::new(pattern)?;

                'blk: {
                    for value in values.as_slice() {
                        if !re.is_match(value) {
                            ::log::log!(
                                level,
                                "pattern /{}/ did not match {:?}",
                                re.as_str(),
                                value
                            );
                            break 'blk failure();
                        }
                    }

                    DependencyResult::Success
                }
            }
        };

        Ok(result)
    }

    /// Visit all parsed string values.
    pub fn visit_strings<E>(
        &mut self,
        mut f: impl FnMut(&mut String) -> Result<(), E>,
    ) -> Result<(), E> {
        match &mut self.kind {
            DependencyKind::Script { id } => f(id),
            DependencyKind::Exec(exec) => exec.visit_strings(f),
            DependencyKind::Equals { values: equals } => equals.iter_mut().try_for_each(f),
            DependencyKind::In {
                values,
                collection: in_collection,
            } => values
                .as_mut_slice()
                .iter_mut()
                .chain(in_collection)
                .try_for_each(f),
            DependencyKind::Matches {
                values,
                pattern: matches,
            } => {
                f(matches)?;
                values.as_mut_slice().iter_mut().try_for_each(f)
            }
        }
    }
}
