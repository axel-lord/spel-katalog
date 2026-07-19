//! Struct for parsing yaml

use ::std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use ::rustc_hash::FxHashMap;
use ::yaml_rust2::{ScanError, Yaml, YamlLoader};

/// A game config.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Game field.
    pub game: Game,
    /// System field.
    pub system: System,
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
    pub fn common_parent(&self, home: fn() -> &'static Path) -> PathBuf {
        fn common(a: &Path, b: &Path) -> PathBuf {
            a.components()
                .zip(b.components())
                .take_while(|(a, b)| a == b)
                .map(|(c, _)| c)
                .collect()
        }

        let prefix = self.prefix.as_deref().unwrap_or_else(home);
        let exe = &self.exe;

        common(exe, prefix)
    }
}

/// System fields.
#[derive(Debug, Clone, Default)]
pub struct System {
    /// Environment variables.
    pub env: FxHashMap<String, String>,
}

/// Yaml item to get game field.
pub static GAME: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("game".into()));
/// Yaml item to get system field.
pub static SYSTEM: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("system".into()));
/// Yaml item to get exe field.
pub static EXE: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("exe".into()));
/// Yaml item to get prefix field.
pub static PREFIX: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("prefix".into()));
/// Yaml item to get arch field.
pub static ARCH: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("arch".into()));
/// Yaml item to get env field.
pub static ENV: LazyLock<Yaml> = LazyLock::new(|| Yaml::String("env".into()));

/// Get exe of game config.
fn get_exe(yml: &Yaml) -> Option<PathBuf> {
    yml.as_hash()?
        .get(&GAME)?
        .as_hash()?
        .get(&EXE)?
        .as_str()
        .map(PathBuf::from)
}

/// get wine prefix of game config.
fn get_prefix(yml: &Yaml) -> Option<PathBuf> {
    yml.as_hash()?
        .get(&GAME)?
        .as_hash()?
        .get(&PREFIX)?
        .as_str()
        .map(PathBuf::from)
}

/// Get wine arch of game config.
fn get_arch(yml: &Yaml) -> Option<String> {
    yml.as_hash()?
        .get(&GAME)?
        .as_hash()?
        .get(&ARCH)?
        .as_str()
        .map(String::from)
}

/// Get environment of config.
fn get_env(yml: &Yaml) -> Option<FxHashMap<String, String>> {
    yml.as_hash()?
        .get(&SYSTEM)?
        .as_hash()?
        .get(&ENV)?
        .as_hash()
        .map(|env| {
            env.into_iter()
                .filter_map(|(key, value)| {
                    Some((key.as_str()?.to_owned(), value.as_str()?.to_owned()))
                })
                .collect()
        })
}

impl Config {
    /// Parse game config.
    ///
    /// # Errors
    /// If the content is not valid yaml.
    pub fn parse(content: &str) -> Result<Self, ScanError> {
        let doc = YamlLoader::load_from_str(content)?;
        Ok(Config {
            game: Game {
                exe: doc.first().and_then(get_exe).unwrap_or_default(),
                prefix: doc.first().and_then(get_prefix),
                arch: doc.first().and_then(get_arch),
            },
            system: System {
                env: doc.first().and_then(get_env).unwrap_or_default(),
            },
        })
    }
}
