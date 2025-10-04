//! Script deps.

use ::std::ops::Not;

use ::bon::Builder;
use ::derive_more::{From, IsVariant};
use ::rustc_hash::FxHashSet;
use ::serde::{Deserialize, Serialize};
use ::spel_katalog_terminal::SinkBuilder;
use ::tap::Tap;

use crate::{
    dependency::failure::failure,
    environment::Env,
    exec::{Exec, ExecError},
    maybe_single::MaybeSingle,
};

mod failure;
mod matches;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum DependencyResult {
    /// Failure which should stop all scripts.
    Panic = 3,
    /// Failure which should only stop current script.
    Failure = 2,
    /// Success which should not run current script.
    Skip = 1,
    /// Success, continue on.
    #[default]
    Success = 0,
}

impl DependencyResult {
    /// Check if the result is of the panic variant.
    ///
    /// This is the only variant check implemented as
    /// most other cases should be handled by match,
    /// (as they might all be considered success), but
    /// there is great utility in checking for the panic
    /// variant, as no more script activity should happen
    /// when returned.
    pub fn is_panic(self) -> bool {
        matches!(self, Self::Panic)
    }
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
    /// Depend on a group of values being unique.
    #[from(ignore)]
    NotEquals {
        /// Values to check if not equal.
        #[serde(rename = "not-equals", alias = "not-equal")]
        values: Vec<String>,
    },
    /// Require a string not be empty.
    #[from(ignore)]
    NotEmpty {
        /// Requre values not be empty.
        #[serde(rename = "not-empty")]
        not_empty: MaybeSingle,
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
    Matches(matches::Matches),
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
        sink_builder: &SinkBuilder,
    ) -> Result<DependencyResult, DependencyError> {
        let Self { kind, panic } = self;

        let result = match kind {
            DependencyKind::Script { id } => {
                let Some(prior) = get_prior(&id) else {
                    return Err(DependencyError::MissingDep(id.clone()));
                };

                match prior {
                    DependencyResult::Panic | DependencyResult::Failure => {
                        failure!(*panic, "script dependency did not succeed, {id}")
                    }
                    DependencyResult::Skip | DependencyResult::Success => DependencyResult::Success,
                }
            }
            DependencyKind::Exec(exec) => {
                let status = exec.run(env, sink_builder).await?;

                if status.success() {
                    DependencyResult::Success
                } else {
                    failure!(*panic, "dependency exec failed, {status}")
                }
            }
            DependencyKind::Equals { values } => match values.as_slice() {
                [] => DependencyResult::Success,
                [head, remainder @ ..] if remainder.iter().all(|e| e == head) => {
                    DependencyResult::Success
                }
                values => failure!(*panic, "equality check failed, values:\n{values:#?}"),
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
                            break 'blk failure!(
                                *panic,
                                "value {value} not in collection\n{collection:#?}"
                            );
                        }
                    }
                    DependencyResult::Success
                }
            }
            DependencyKind::Matches(m) => m.check(*panic).await?,
            DependencyKind::NotEquals { values } => {
                if values
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<FxHashSet<&str>>()
                    .len()
                    == values.len()
                {
                    DependencyResult::Success
                } else {
                    failure!(*panic, "inequality check failed, values:\n{values:#?}")
                }
            }
            DependencyKind::NotEmpty { not_empty } => 'blk: {
                for value in not_empty.as_slice() {
                    if value.is_empty() {
                        break 'blk failure!(*panic, "a not-empty requirement is empty");
                    }
                }
                DependencyResult::Success
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
            DependencyKind::In { values, collection } => values
                .as_mut_slice()
                .iter_mut()
                .chain(collection)
                .try_for_each(f),
            DependencyKind::Matches(m) => m.visit_strings(&mut f),
            DependencyKind::NotEquals { values } => values.iter_mut().try_for_each(f),
            DependencyKind::NotEmpty { not_empty } => {
                not_empty.as_mut_slice().iter_mut().try_for_each(f)
            }
        }
    }
}
