//! [InstallerConfig] impl.

use ::std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use ::serde::{Deserialize, Serialize};

/// Arguments passed to installer prefill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerConfig {
    /// Relative parent of exe choices.
    pub game_dir: PathBuf,
    /// Exe choice,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<PathBuf>,
    /// Is the game hidden, shown or undecided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Path to thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PathBuf>,
    /// Should the game be moved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_game: Option<bool>,
}

impl InstallerConfig {
    /// Convert into a [InstallerPrepareConfig] using `choice_finder` to
    /// find exes from `game_dir` if missing.
    pub async fn into_prepare_config<C, F>(self, choice_finder: C) -> Option<InstallerPrepareConfig>
    where
        C: FnOnce(PathBuf) -> F,
        F: Future<Output = Option<(String, ExeChoice)>>,
    {
        let Self {
            game_dir,
            exe,
            hidden,
            thumbnail,
            move_game,
        } = self;
        let (parent, choice) = if let Some(exe) = exe {
            (
                game_dir
                    .into_os_string()
                    .into_string()
                    .map_err(|err| ::log::error!("could not convert {err:?} to utf-8"))
                    .ok()?,
                ExeChoice::Value(
                    exe.into_os_string()
                        .into_string()
                        .map_err(|err| ::log::error!("could not convert {err:?} to utf-8"))
                        .ok()?,
                ),
            )
        } else {
            choice_finder(game_dir).await?
        };

        Some(InstallerPrepareConfig {
            parent,
            choice,
            hidden,
            thumbnail,
            move_game,
        })
    }
}

/// Arguments passed to installer window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerPrepareConfig {
    /// Relative parent of exe choices.
    pub parent: String,
    /// Exe choice/s.
    pub choice: ExeChoice,
    /// Is the game hidden, shown or undecided.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Path to thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<PathBuf>,
    /// Should the game be moved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_game: Option<bool>,
}

/// Choice of executable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExeChoice {
    /// A single exe is chosen.
    /// If not representable by a string
    /// lossy conversion is performed and the
    /// original path is included.
    Value(String),
    /// A list of executables that are available.
    /// Every entry has the same format as [Self::Value]
    /// The first value is the index of the chosen candidate.
    List(usize, Vec<String>),
}

impl ExeChoice {
    /// get current choice.
    pub fn current(&self) -> Option<&str> {
        match self {
            ExeChoice::Value(exe) => Some(exe),
            ExeChoice::List(idx, items) => items.get(*idx).as_ref().map(|s| s.as_str()),
        }
    }

    /// Get file extension of selected choice.
    pub fn extension(&self) -> Option<&str> {
        Path::new(self.current()?)
            .extension()
            .and_then(OsStr::to_str)
    }

    /// Check if choice has given extension.
    pub fn has_ext(&self, ext: &str) -> bool {
        self.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    }
}
