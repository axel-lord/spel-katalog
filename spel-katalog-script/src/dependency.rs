//! Script deps.

use ::std::ops::Not;

use ::bon::Builder;
use ::derive_more::{From, IsVariant};
use ::serde::{Deserialize, Serialize};

use crate::{
    environment::Env,
    exec::{Exec, RunError},
};

/// Error type returned by dependency check.
#[derive(Debug, ::thiserror::Error)]
pub enum DependencyError {
    /// Running an executable failed.
    #[error(transparent)]
    RunExec(#[from] RunError),
    /// A Script dependency was not ran before this one.
    #[error("no result available for {0:?}")]
    MissingDep(String),
}

/// The result of a dependency check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
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

/// Different kinds of dependency.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, From, IsVariant)]
#[serde(untagged)]
pub enum DependencyKind {
    /// Depend on the existance/success of a scripts dependencies.
    #[from(ignore)]
    Script {
        /// Id of dependency.
        id: String,
    },
    /// Depend on the success of an executable.
    Exec(Exec),
}

impl DependencyKind {
    /// Create a script dependency on the given id.
    pub fn id(id: impl Into<String>) -> Self {
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
        self,
        env: &Env,
        get_prior: impl for<'k> FnOnce(&'k str) -> Option<DependencyResult>,
    ) -> Result<DependencyResult, DependencyError> {
        let Self { kind, try_dep } = self;

        let success = match kind {
            DependencyKind::Script { id } => {
                let Some(prior) = get_prior(&id) else {
                    return Err(DependencyError::MissingDep(id));
                };

                match prior {
                    DependencyResult::Success => true,
                    _ if try_dep => {
                        ::log::info!("dependency did not succeed (try), {id}");
                        false
                    }
                    _ => {
                        ::log::error!("dependency did not succeed, {id}");
                        false
                    }
                }
            }
            DependencyKind::Exec(exec) => {
                let status = exec.run(env).await?;

                if status.success() {
                    true
                } else if try_dep {
                    ::log::info!("dependency exec failed (try), {status}");
                    false
                } else {
                    ::log::error!("dependency exec failed, {status}");
                    false
                }
            }
        };

        match success {
            false if try_dep => Ok(DependencyResult::TryFailure),
            false => Ok(DependencyResult::Failure),
            true => Ok(DependencyResult::Success),
        }
    }

    /// Visit all parsed string values.
    pub fn visit_strings<E>(
        &mut self,
        mut f: impl FnMut(&mut String) -> Result<(), E>,
    ) -> Result<(), E> {
        match &mut self.kind {
            DependencyKind::Script { id } => f(id),
            DependencyKind::Exec(exec) => exec.visit_strings(f),
        }
    }
}
