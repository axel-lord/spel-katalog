//! Struct for parsing yaml

use ::std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use ::rustc_hash::FxHashMap;
use ::serde::{Deserialize, Serialize};
use ::yaml_rust2::{ScanError, Yaml, YamlLoader};

/// A game config.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Game field.
    pub game: Game,
}

/// Game fields.
#[derive(Debug, Clone, Default)]
pub struct Game {
    /// Exe path.
    pub exe: PathBuf,
    /// Prefix path.
    pub prefix: Option<PathBuf>,
    /// Game wine arch.
    pub arch: Option<String>,
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

static GAME: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("game".into()));
static EXE: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("exe".into()));
static PREFIX: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("prefix".into()));
static ARCH: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("arch".into()));

fn get_exe(yml: &Yaml) -> Option<PathBuf> {
    yml.as_hash()?
        .get(&GAME)?
        .as_hash()?
        .get(&EXE)?
        .as_str()
        .map(PathBuf::from)
}

fn get_prefix(yml: &Yaml) -> Option<PathBuf> {
    yml.as_hash()?
        .get(&GAME)?
        .as_hash()?
        .get(&PREFIX)?
        .as_str()
        .map(PathBuf::from)
}

fn get_arch(yml: &Yaml) -> Option<String> {
    yml.as_hash()?
        .get(&GAME)?
        .as_hash()?
        .get(&ARCH)?
        .as_str()
        .map(String::from)
}

impl Config {
    pub fn parse(content: &str) -> Result<Self, ScanError> {
        let doc = YamlLoader::load_from_str(content)?;
        Ok(Config {
            game: Game {
                exe: doc.get(0).and_then(get_exe).unwrap_or_default(),
                prefix: doc.get(0).and_then(get_prefix),
                arch: doc.get(0).and_then(get_arch),
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Additional {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub sandbox_root: Vec<String>,

    #[serde(skip_serializing_if = "FxHashMap::is_empty", default)]
    pub attrs: FxHashMap<String, String>,
}
