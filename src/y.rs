use ::std::path::{Path, PathBuf};

use ::serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub game: Game,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Game {
    pub exe: PathBuf,
    pub prefix: Option<PathBuf>,
}

impl Game {
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
            .unwrap_or_else(|| crate::settings::HOME.as_path());
        let exe = &self.exe;

        common(exe, prefix)
    }
}
