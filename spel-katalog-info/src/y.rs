//! Struct for parsing yaml

use ::std::path::{Path, PathBuf};

use ::serde::Deserialize;

/// A game config.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Game field.
    pub game: Game,
}

/// Game fields.
#[derive(Debug, Clone, Deserialize)]
pub struct Game {
    /// Exe path.
    pub exe: PathBuf,
    /// Prefix path.
    pub prefix: Option<PathBuf>,
}

impl Game {
    /// Get the common parent of exe and prefix.
    pub fn common_parent(&self) -> PathBuf {
        fn common(a: &Path, b: &Path) -> PathBuf {
            a.components()
                .zip(b.components())
                .take_while(|(a, b)| a == b)
                .map(|(c, _)| c)
                .collect()
        }

        let prefix = self
            .prefix
            .as_deref()
            .unwrap_or_else(|| ::spel_katalog_settings::HOME.as_path());
        let exe = &self.exe;

        common(exe, prefix)
    }
}
