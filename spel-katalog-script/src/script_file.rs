//! Script file contents

use ::bon::Builder;
use ::serde::{Deserialize, Serialize};

use crate::{
    builder_push::builder_push, dependency::Dependency, environment::Env, exec::Exec,
    script::Script,
};

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
}

builder_push! {
    ScriptFileBuilder
    { assert: Dependency }
    { require: Dependency }
    { parallell: impl Into<Exec> => parallell.into() }
    { synced: impl Into<Exec> => synced.into() }
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
