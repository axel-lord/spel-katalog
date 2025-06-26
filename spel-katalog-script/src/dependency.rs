//! Script deps.

use ::std::ops::Not;

use ::bon::Builder;
use ::derive_more::{From, IsVariant};
use ::serde::{Deserialize, Serialize};
use ::tap::Tap;

use crate::{
    environment::Env,
    exec::{Exec, ExecError},
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
    #[serde(default, skip_serializing_if = "Not::not", rename = "try")]
    #[builder(default)]
    pub try_dep: bool,
}

/// Value/s for in check.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, From, IsVariant)]
#[serde(untagged)]
pub enum MaybeSingle {
    /// Multiple values.
    Multiple(Vec<String>),
    /// A Single value.
    Single(String),
}

impl MaybeSingle {
    /// Get contents as a slice.
    pub fn as_slice(&self) -> &[String] {
        match self {
            MaybeSingle::Multiple(values) => values,
            MaybeSingle::Single(value) => ::std::slice::from_ref(value),
        }
    }

    /// Get contents as a slice.
    pub fn as_mut_slice(&mut self) -> &mut [String] {
        match self {
            MaybeSingle::Multiple(values) => values,
            MaybeSingle::Single(value) => ::std::slice::from_mut(value),
        }
    }

    /// Sort and deduplicate contents
    pub fn dedup(mut self) -> Self {
        if let Self::Multiple(values) = &mut self {
            values.sort_unstable();
            values.dedup();
        }
        self
    }
}

/// Different kinds of dependency.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, From, IsVariant)]
#[serde(untagged)]
pub enum DependencyKind {
    /// Depend on the existance/success of a scripts dependencies.
    #[from(ignore)]
    Script {
        /// Id of dependency.
        script: String,
    },
    /// Depend on the success of an executable.
    Exec(Exec),
    /// Depend on a group of values being the same.
    ValueEquals {
        /// Values to check if equal.
        equals: Vec<String>,
    },
    /// Depend on a value being in a list.
    ValeuIn {
        /// Values to check for.
        #[serde(alias = "value")]
        values: MaybeSingle,
        /// Collection to check for values in.
        #[serde(rename = "in")]
        in_collection: Vec<String>,
    },
}

impl DependencyKind {
    /// Create a script dependency on the given id.
    pub fn script(id: impl Into<String>) -> Self {
        Self::Script { script: id.into() }
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
        let Self { kind, try_dep } = self;

        let result = match kind {
            DependencyKind::Script { script: id } => {
                let Some(prior) = get_prior(&id) else {
                    return Err(DependencyError::MissingDep(id.clone()));
                };

                match prior {
                    result @ DependencyResult::Success => result,
                    _ if *try_dep => {
                        ::log::info!("dependency did not succeed (try), {id}");
                        DependencyResult::TryFailure
                    }
                    _ => {
                        ::log::error!("dependency did not succeed, {id}");
                        DependencyResult::Failure
                    }
                }
            }
            DependencyKind::Exec(exec) => {
                let status = exec.run(env).await?;

                if status.success() {
                    DependencyResult::Success
                } else if *try_dep {
                    ::log::info!("dependency exec failed (try), {status}");
                    DependencyResult::TryFailure
                } else {
                    ::log::error!("dependency exec failed, {status}");
                    DependencyResult::Failure
                }
            }
            DependencyKind::ValueEquals { equals } => match equals.as_slice() {
                [head, remainder @ ..] if remainder.iter().all(|e| e == head) => {
                    DependencyResult::Success
                }
                [] => DependencyResult::Success,
                values if *try_dep => {
                    ::log::info!("equality check failed (try), for values\n{values:#?}");
                    DependencyResult::TryFailure
                }
                values => {
                    ::log::error!("equality check failed, for values\n{values:#?}");
                    DependencyResult::Failure
                }
            },
            DependencyKind::ValeuIn {
                values,
                in_collection,
            } => {
                let values = values.clone().dedup();
                let in_collection = in_collection.clone().tap_mut(|in_collection| {
                    in_collection.sort_unstable();
                    in_collection.dedup();
                });

                'blk: {
                    for value in values.as_slice() {
                        if in_collection.binary_search(&value).is_ok() {
                            continue;
                        }

                        break 'blk if *try_dep {
                            ::log::info!(
                                "value {value} not in collection (try)\n{in_collection:#?}"
                            );
                            DependencyResult::TryFailure
                        } else {
                            ::log::error!("value {value} not in collection\n{in_collection:#?}");
                            DependencyResult::Failure
                        };
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
            DependencyKind::Script { script: id } => f(id),
            DependencyKind::Exec(exec) => exec.visit_strings(f),
            DependencyKind::ValueEquals { equals } => equals.iter_mut().try_for_each(f),
            DependencyKind::ValeuIn {
                values,
                in_collection,
            } => values
                .as_mut_slice()
                .iter_mut()
                .chain(in_collection)
                .try_for_each(f),
        }
    }
}
