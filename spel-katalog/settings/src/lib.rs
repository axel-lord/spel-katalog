//! Settings widgets.

use ::clap::Args;
use ::derive_more::{From, IsVariant};
use ::iced_core::{Alignment, Element, Length::Fill};
use ::iced_runtime::Task;
use ::iced_widget::{button, space, text};

use ::core::ops::{Deref, DerefMut};
use ::spel_katalog_common::{StatusSender, async_status, w};
use ::std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use ::tap::Pipe;

pub use ::spel_katalog_settings_traits::*;

mod environment;
mod list;

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

impl From<Theme> for ::iced_core::Theme {
    fn from(value: Theme) -> Self {
        macro_rules! themes {
            ($v:expr, $($theme:ident,)*) => {
                match $v {$(
                    Theme:: $theme => ::iced_core::Theme:: $theme,
                )*}
            };
        }
        themes!(
            value,
            Light,
            Dark,
            SolarizedLight,
            SolarizedDark,
            GruvboxLight,
            GruvboxDark,
            Dracula,
            Nord,
            CatppuccinLatte,
            CatppuccinFrappe,
            CatppuccinMacchiato,
            CatppuccinMocha,
            TokyoNight,
            TokyoNightStorm,
            TokyoNightLight,
            KanagawaWave,
            KanagawaDragon,
            KanagawaLotus,
            Moonfly,
            Nightfly,
            Oxocarbon,
            Ferra,
        )
    }
}

/// Message used by settings view.
#[derive(Debug, IsVariant, Clone, From)]
pub enum Message {
    /// Apply a settings change.
    Delta(Delta),
    /// Save settings.
    Save,
}

/// State of settings view.
#[derive(Debug, Clone)]
pub struct State {
    /// Settings state.
    pub settings: Settings,
    /// Path to config file.
    pub config: PathBuf,
}

impl DerefMut for State {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.settings
    }
}

impl Deref for State {
    type Target = Settings;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}

/// Save settings to given path.
///
/// # Errors
/// If settings cannot be either serialized or saved.
async fn save(settings: Settings, path: PathBuf) -> Result<PathBuf, PathBuf> {
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

impl State {
    /// Apply delta created from t to self.
    pub fn apply_from<T>(&mut self, t: T)
    where
        Delta: From<T>,
    {
        Delta::from(t).apply(self);
    }

    /// Get a snapshot of settings at time ov invocation.
    pub fn snapshot(&self) -> Settings {
        self.settings.clone()
    }

    /// Update state by message.
    pub fn update(&mut self, message: Message, tx: &StatusSender) -> Task<Message> {
        match message {
            Message::Delta(delta) => {
                delta.apply(self);
            }
            Message::Save => {
                let tx = tx.clone();
                let settings = self.snapshot();
                let path = self.config.clone();
                return Task::future(async move {
                    match save(settings, path).await {
                        Ok(path) => async_status!(tx, "saved settings to {path:?}").await,
                        Err(path) => async_status!(tx, "could not save settings to {path:?}").await,
                    }
                })
                .then(|_| Task::none());
            }
        };
        Task::none()
    }

    /// View settings.
    pub fn view(&self) -> Element<'_, Message, ::iced_core::Theme, ::iced_renderer::Renderer> {
        w::col()
            .align_x(Alignment::Start)
            .width(Fill)
            .push(
                w::row()
                    .width(Fill)
                    .push(text("Settings").align_x(Alignment::Center).width(Fill))
                    .push(button("Save").padding(3).on_press(Message::Save)),
            )
            .push(spel_katalog_widget::rule::horizontal())
            .push(
                self.view_enums()
                    .map(Message::Delta)
                    .pipe(::spel_katalog_widget::scrollable),
            )
            .push(spel_katalog_widget::rule::horizontal())
            .push(
                self.view_paths()
                    .push(space())
                    .pipe(Element::from)
                    .map(Message::Delta)
                    .pipe(::spel_katalog_widget::scrollable),
            )
            .into()
    }
}
