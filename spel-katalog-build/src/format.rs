//! Structured input format.
use ::std::{fs, path::Path};

use ::indexmap::IndexMap;
use ::rustc_hash::FxBuildHasher;
use ::serde::Deserialize;

use crate::format::settings::Setting;

/// The content of a settings spec file.
#[derive(Debug, Deserialize)]
pub struct Settings {
    /// Settings to generate code for.
    #[serde(flatten)]
    pub settings: IndexMap<String, Setting, FxBuildHasher>,
}

impl Settings {
    /// Read a settings struct from a path.
    ///
    /// # Panics
    /// Should the settings at path be malformed.
    pub fn read(path: &Path) -> Self {
        ::toml::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }
}

pub mod settings {
    //! Inner types for settings.

    use ::serde::Deserialize;

    /// The differing content of a Single setting.
    #[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    #[serde(untagged)]
    pub enum SettingContent {
        /// Setting is an enum.
        Enum {
            /// Setting variants.
            variants: Vec<String>,
            /// Default variant of setting.
            default: String,
        },
        /// Setting is a path.
        Path {
            /// Setting default path.
            path: String,
        },
    }

    /// A Single setting.
    #[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Setting {
        /// Title of setting.
        pub title: Option<String>,
        /// Help message of setting.
        pub help: String,
        /// Content of setting.
        #[serde(flatten)]
        pub content: SettingContent,
    }
}
