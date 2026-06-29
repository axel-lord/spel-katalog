//! Settings widgets.

use ::clap::Args;

use ::core::ops::{Deref, DerefMut};
use ::std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

pub use ::spel_katalog_settings_traits::*;

mod environment;

#[doc(hidden)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/settings.rs"));
    pub use crate::environment::*;
}
pub use generated::*;

/// Command line arguments for settings.
#[derive(Debug, Args, Default, Clone)]
pub struct SettingsArgs {
    /// Settings arguments.
    #[command(flatten)]
    args: SettingsStore,
}

impl SettingsArgs {
    /// Get xdg base directories for arguments.
    fn get_xdg(&self) -> ::xdg::BaseDirectories {
        ::xdg::BaseDirectories::with_prefix("spel-katalog")
    }
}

/// Settings storage.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Inner settings stored.
    inner: Arc<SettingsStore>,
    /// Xdg base directories.
    xdg: Arc<::xdg::BaseDirectories>,
}

impl Settings {
    /// Get option by type
    pub fn get<T>(&self) -> &T::Output
    where
        T: AsIndex<SettingsStore>,
    {
        T::as_idx().get(&self.inner)
    }

    /// Get mutable option by type
    pub fn get_mut<T>(&mut self) -> &mut T::Output
    where
        T: AsIndex<SettingsStore>,
    {
        T::as_idx().get_mut(Arc::make_mut(&mut self.inner))
    }

    /// Get xdg base directories.
    pub fn xdg(&self) -> &::xdg::BaseDirectories {
        &self.xdg
    }
}

impl From<SettingsArgs> for Settings {
    fn from(value: SettingsArgs) -> Self {
        Self {
            xdg: Arc::new(value.get_xdg()),
            inner: Arc::new(value.args),
        }
    }
}

impl Deref for Settings {
    type Target = SettingsStore;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Settings {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.inner)
    }
}

/// A generic representation of current settings.
pub type Generic = HashMap<&'static str, String>;

impl<T> ::core::ops::Index<T> for Settings
where
    T: SettingsIndex<SettingsStore>,
{
    type Output = T::Output;

    fn index(&self, index: T) -> &Self::Output {
        index.get(&self.inner)
    }
}

impl<T> ::core::ops::IndexMut<T> for Settings
where
    T: SettingsIndexMut<SettingsStore>,
{
    fn index_mut(&mut self, index: T) -> &mut Self::Output {
        index.get_mut(Arc::make_mut(&mut self.inner))
    }
}

/// Save settings to given path.
///
/// # Errors
/// If settings cannot be either serialized or saved.
pub async fn save(settings: Settings, path: PathBuf) -> Result<PathBuf, PathBuf> {
    match ::toml::to_string_pretty(&*settings.inner) {
        Ok(contents) => match ::smol::fs::write(&path, contents).await {
            Ok(_) => Ok(path),
            Err(err) => {
                ::log::error!("could not write settings to {path:?}\n{err}");
                Err(path)
            }
        },
        Err(err) => {
            ::log::error!("could not serialize settings\n{err}");
            Err(path)
        }
    }
}

/// Load settings from given path, with specified overrides.
pub fn load(path: &Path, overrides: SettingsArgs) -> Settings {
    fn read_settings(config: &Path) -> Result<SettingsStore, ()> {
        let content = ::std::fs::read_to_string(config).map_err(|err| {
            ::log::warn!("could not read {config:?}, does it exists an is it readable?\n{err}");
        })?;

        ::toml::from_str(&content).map_err(|err| {
            ::log::warn!("could not parse {config:?} as toml, is it a toml file?\n{err}")
        })
    }

    Settings {
        xdg: Arc::new(overrides.get_xdg()),
        inner: Arc::new(
            read_settings(path)
                .unwrap_or_default()
                .apply(Delta::create(overrides.args)),
        ),
    }
}
