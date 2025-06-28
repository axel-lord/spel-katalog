//! Script file contents

use ::std::{
    borrow::Cow,
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::ExitStatus,
};

use ::bon::Builder;
use ::futures::{stream::FuturesUnordered, stream::StreamExt};
use ::rustc_hash::FxHashMap;
use ::serde::{Deserialize, Serialize};
use ::tap::{Pipe, Tap, TryConv};

use crate::{
    builder_push::builder_push,
    dependency::{Dependency, DependencyError, DependencyResult},
    environment::Env,
    exec::{Exec, ExecError},
    script::Script,
};

/// Error type returned by running of script files.
#[derive(Debug, ::thiserror::Error)]
pub enum RunError {
    /// An executable could not be ran.
    #[error(transparent)]
    RunExec(#[from] ExecError),
    /// A dependency check could not be ran.
    #[error(transparent)]
    Dependency(#[from] DependencyError),
    /// A dependency check returned a non try failure.
    #[error("dependency check failed for {0}")]
    DepCheck(String),
    /// An executable returned a non-success status.
    #[error("script {0} returned {1}")]
    ExitStatus(String, ExitStatus),
}

/// Error returned when failing to parse/read a script file.
#[derive(Debug, ::thiserror::Error)]
pub enum ReadError {
    /// Failed to parse a toml file.
    #[error(transparent)]
    Toml(#[from] ::toml::de::Error),

    /// Failed to parse a json file.
    #[error(transparent)]
    Json(#[from] ::serde_json::Error),

    /// Io error occurred while reading.
    #[error(transparent)]
    Read(#[from] ::std::io::Error),

    /// Extension was not toml or json.
    #[error("extension {0:?} should be \"toml\" or \"json\"")]
    UnknownExtension(OsString),
}

/// A script specification.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq, Builder)]
pub struct ScriptFile {
    /// Dependencies checked before script is ran. Any script dependencies will also be
    /// checked by require, to avoid needing duplication, and this will check for success.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(field)]
    pub assert: Vec<Dependency>,

    /// Dependencies checked before any script is ran.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(field)]
    pub require: Vec<Dependency>,

    /// Items to execute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(field)]
    pub synced: Vec<Exec>,

    /// Items to execute in any order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(field)]
    pub parallell: Vec<Exec>,

    /// Script section of file.
    pub script: Script,

    /// Application environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Env>,

    /// Path to parsed file.
    #[serde(skip)]
    pub source: Option<PathBuf>,
}

builder_push! {
    ScriptFileBuilder
    { assert: Dependency }
    { require: Dependency }
    { parallell: impl Into<Exec> => parallell.into() }
    { synced: impl Into<Exec> => synced.into() }
}

fn ref_or_default<T: Clone + Default>(t: Option<&T>) -> Cow<T> {
    match t {
        Some(t) => Cow::Borrowed(t),
        None => Cow::default(),
    }
}

impl ScriptFile {
    /// Parse json into a `ScriptFile`.
    pub fn from_json(json: &str) -> Result<Self, ::serde_json::Error> {
        ::serde_json::from_str(json)
    }

    /// Parse toml into a `ScriptFile`.
    pub fn from_toml(toml: &str) -> Result<Self, ::toml::de::Error> {
        ::toml::from_str(toml)
    }

    /// Read a script from a path.
    pub async fn read(path: &Path) -> Result<Self, ReadError> {
        let ext = path
            .extension()
            .ok_or_else(|| ReadError::UnknownExtension(OsString::new()))?;

        let ext = ext
            .as_bytes()
            .try_conv::<[u8; 4]>()
            .map_err(|_| ReadError::UnknownExtension(ext.to_os_string()))?
            .tap_mut(|ext| ext.make_ascii_uppercase());

        let content = ::tokio::fs::read_to_string(path);
        let mut script_file = match &ext {
            b"TOML" => {
                let script = Self::from_toml(&content.await?)?;
                Ok(script)
            }
            b"JSON" => {
                let script = Self::from_json(&content.await?)?;
                Ok(script)
            }

            ext => OsStr::from_bytes(ext)
                .to_os_string()
                .pipe(ReadError::UnknownExtension)
                .pipe(Err),
        }?;
        script_file.source = Some(path.to_owned());
        Ok(script_file)
    }

    /// Get the id of a script file.
    pub fn id(&self) -> &str {
        &self.script.id
    }

    /// Requirement checks ran before script,
    /// includes `require` and early script deps of `assert`.
    pub async fn pre_run_check<'a>(
        scripts: &'a [ScriptFile],
    ) -> Result<(FxHashMap<&'a str, DependencyResult>, Vec<&'a ScriptFile>), RunError> {
        let mut results = FxHashMap::default();
        let mut passed_early = Vec::new();

        for script_file in scripts {
            let id = script_file.id();

            // Script ids which have already succeeded are skipped.
            if let Some(DependencyResult::Success) = results.get(id) {
                continue;
            }

            // Check if dependency holds.
            let result = script_file
                .check_require(|id| results.get(id).copied())
                .await?;

            // A non try error interrupts all further processing.
            if result.is_failure() {
                return Err(RunError::DepCheck(id.to_owned()));
            }

            // The result is inserted into the map, and the script file is passed to the next step.
            results.insert(id, result);
            passed_early.push(script_file);
        }

        // At post require step results are not stored.
        // as we only care about non try failures and check errors.
        let mut passed_late = Vec::new();
        for script_file in passed_early {
            let result = script_file
                .check_post_require(|id| results.get(id).copied())
                .await?;

            if result.is_failure() {
                return Err(RunError::DepCheck(script_file.id().to_owned()));
            }
            passed_late.push(script_file);
        }

        Ok((results, passed_late))
    }

    /// Run this script, only checks assert requirements.
    ///
    /// Returns the result of assert requirements.
    pub async fn run(
        &self,
        get_prior: impl for<'k> Fn(&'k str) -> Option<DependencyResult>,
    ) -> Result<DependencyResult, RunError> {
        let Self {
            synced,
            parallell,
            script,
            env,
            ..
        } = self;
        let id = self.id();
        let result = get_prior(id)
            .ok_or_else(|| RunError::DepCheck(id.to_owned()))?
            .max(self.check_assert(&get_prior).await?);

        match result {
            DependencyResult::Failure => return Err(RunError::DepCheck(id.to_owned())),
            DependencyResult::TryFailure => return Ok(DependencyResult::TryFailure),
            DependencyResult::Success => {}
        }

        let env = ref_or_default(env.as_ref());

        for exec in script.exec.as_ref().into_iter().chain(synced) {
            let status = exec.run(&env).await?;

            if !status.success() {
                return Err(RunError::ExitStatus(id.to_owned(), status));
            }
        }

        let mut futures = parallell
            .iter()
            .map(|exec| exec.run(&env))
            .collect::<FuturesUnordered<_>>();

        while let Some(status) = futures.next().await {
            let status = status?;
            if !status.success() {
                return Err(RunError::ExitStatus(id.to_owned(), status));
            }
        }

        Ok(result)
    }

    /// Run multiple script files.
    pub async fn run_all(scripts: &[ScriptFile]) -> Result<(), RunError> {
        let (mut results, scripts) = Self::pre_run_check(scripts).await?;

        for script_file in scripts {
            let result = script_file.run(|id| results.get(id).copied()).await?;
            results.insert(script_file.id(), result);
        }

        Ok(())
    }

    async fn check_multiple(
        &self,
        values: impl IntoIterator<Item = &Dependency>,
        get_prior: impl for<'k> Fn(&'k str) -> Option<DependencyResult>,
    ) -> Result<DependencyResult, DependencyError> {
        let env;
        let env = if let Some(env) = &self.env {
            env
        } else {
            env = Env::default();
            &env
        };

        let mut futures = values
            .into_iter()
            .map(|dep| dep.check(env, &get_prior))
            .collect::<FuturesUnordered<_>>();

        let mut result = DependencyResult::Success;
        while let Some(dep_result) = futures.next().await {
            result = result.max(dep_result?);
            if result.is_failure() {
                return Ok(result);
            }
        }

        Ok(result)
    }

    /// Check that require dependencies hold.
    ///
    /// # Errors
    /// If the check cannot be performed.
    pub async fn check_require(
        &self,
        get_prior: impl for<'k> Fn(&'k str) -> Option<DependencyResult>,
    ) -> Result<DependencyResult, DependencyError> {
        self.check_multiple(&self.require, get_prior).await
    }

    /// Check that early assert dependencies hold.
    ///
    /// # Errors
    /// If the check cannot be performed.
    pub async fn check_post_require(
        &self,
        get_prior: impl for<'k> Fn(&'k str) -> Option<DependencyResult>,
    ) -> Result<DependencyResult, DependencyError> {
        self.check_multiple(
            self.assert.iter().filter(|dep| dep.kind.is_script()),
            get_prior,
        )
        .await
    }

    /// Check that assert dependencies hold.
    ///
    /// # Errors
    /// If the check cannot be performed.
    pub async fn check_assert(
        &self,
        get_prior: impl for<'k> Fn(&'k str) -> Option<DependencyResult>,
    ) -> Result<DependencyResult, DependencyError> {
        self.check_multiple(&self.assert, get_prior).await
    }

    /// Visit all parsed string values. Notably not environment variable keys.
    pub fn visit_strings<E>(
        &mut self,
        mut f: impl FnMut(&mut String) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            assert,
            require,
            synced,
            parallell,
            script,
            env,
            source: _,
        } = self;

        assert
            .iter_mut()
            .try_for_each(|a| a.visit_strings(&mut f))?;
        require
            .iter_mut()
            .try_for_each(|a| a.visit_strings(&mut f))?;
        synced
            .iter_mut()
            .try_for_each(|e| e.visit_strings(&mut f))?;
        parallell
            .iter_mut()
            .try_for_each(|e| e.visit_strings(&mut f))?;
        env.iter_mut().try_for_each(|e| e.visit_strings(&mut f))?;

        script.visit_strings(f)
    }
}
