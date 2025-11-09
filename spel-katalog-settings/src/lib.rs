//! Settings widgets.

use ::derive_more::{Deref, DerefMut, From, IsVariant};
use ::iced::{
    Alignment, Element,
    Length::Fill,
    Task,
    widget::{button, horizontal_rule, horizontal_space, text},
};

use ::spel_katalog_common::{StatusSender, async_status, w};
use ::std::{collections::HashMap, path::PathBuf};
use ::tap::Pipe;

mod list;

mod environment {
    //! Environment available for settings.
    use ::spel_katalog_common::lazy::Lazy;

    /// Remove trailing newlines.
    fn remove_trailing(mut s: String) -> String {
        let len = s.trim_end_matches('/').len();
        s.truncate(len);
        s
    }

    /// Value for HOME.
    fn home() -> String {
        ::std::env::var("HOME").map_or_else(
            |err| {
                ::log::warn!("could not get home directory, {err}");
                format!("/tmp/spel-katalog.{}", ::whoami::username())
            },
            remove_trailing,
        )
    }

    /// Value fo CONFIG.
    fn config() -> String {
        ::std::env::var("XDG_CONFIG_HOME").map_or_else(
            |err| {
                ::log::warn!("could not get config directory, {err}");
                format!("{HOME}/.config")
            },
            remove_trailing,
        )
    }

    /// Value for CACHE.
    fn cache() -> String {
        ::std::env::var("XDG_CACHE_HOME").map_or_else(
            |err| {
                ::log::warn!("could not get cache directory, {err}");
                format!("{HOME}/.cache")
            },
            remove_trailing,
        )
    }

    /// Value for DATA.
    fn data() -> String {
        ::std::env::var("XDG_DATA_HOME").map_or_else(
            |err| {
                ::log::warn!("could not get data directory, {err}");
                format!("{HOME}/.local/share")
            },
            remove_trailing,
        )
    }

    /// Value for STATE.
    fn state() -> String {
        ::std::env::var("XDG_STATE_HOME").map_or_else(
            |err| {
                ::log::warn!("could not get state directory, {err}");
                format!("{HOME}/.local/state")
            },
            remove_trailing,
        )
    }

    /// User home directory.
    pub static HOME: Lazy = Lazy::new(home);

    /// User config directory, defaults to `~/.config`.
    pub static CONFIG: Lazy = Lazy::new(config);

    /// User config directory, defaults to `~/.cache`.
    pub static CACHE: Lazy = Lazy::new(cache);

    /// User config directory, defaults to `~/.local/share`.
    pub static DATA: Lazy = Lazy::new(data);

    /// User config directory, defaults to `~/.local/state`.
    pub static STATE: Lazy = Lazy::new(state);
}

#[doc(hidden)]
mod generated {
    #![allow(missing_docs)]

    pub use crate::environment::*;

    impl Settings {
        /// Get option by type
        pub fn get<T: super::AsIndex>(&self) -> &T::Output {
            &self[T::as_idx()]
        }

        /// Get mutable option by type
        pub fn get_mut<T: super::AsIndex>(&mut self) -> &mut T::Output {
            &mut self[T::as_idx()]
        }
    }

    include!(concat!(env!("OUT_DIR"), "/settings.rs"));
}
pub use generated::*;

/// A generic representation of current settings.
pub type Generic = HashMap<&'static str, String>;

/// Trait to provide a default string representation of a type.
pub trait DefaultStr {
    /// Get the default string representation of self.
    fn default_str() -> &'static str;
}

/// Trait to provide titles for settings
pub trait Title {
    /// Title to use for setting.
    fn title() -> &'static str;
}

/// Trait to provide help for settings.
pub trait Help {
    /// Get help for setting.
    fn help() -> &'static str;
}

/// Trait for types which index settings.
pub trait SettingsIndex {
    /// Output type returned by indexing
    type Output: ?Sized;

    /// Get the output type.
    fn get(self, settings: &Settings) -> &Self::Output;
}

/// Trait for types wich index settings.
pub trait SettingsIndexMut
where
    Self: SettingsIndex,
{
    /// Get the output type as mutable.
    fn get_mut(self, settings: &mut Settings) -> &mut Self::Output;
}

/// Trait for types which may supply an index type.
pub trait AsIndex {
    /// Output type of index operation.
    type Output;
    /// Supply the index.
    fn as_idx() -> impl SettingsIndexMut<Output = Self::Output>;
}

impl<T> ::core::ops::Index<T> for Settings
where
    T: SettingsIndex,
{
    type Output = T::Output;

    fn index(&self, index: T) -> &Self::Output {
        index.get(self)
    }
}

impl<T> ::core::ops::IndexMut<T> for Settings
where
    T: SettingsIndexMut,
{
    fn index_mut(&mut self, index: T) -> &mut Self::Output {
        index.get_mut(self)
    }
}

/// Trait for simple enums to provide all values.
///
/// # Safety
/// The `VARIANTS` associated constant must contain all variants.
pub unsafe trait Variants
where
    Self: 'static + Sized,
{
    /// All values for enum.
    const VARIANTS: &[Self];

    /// Select the next variant.
    fn cycle(&self) -> Self
    where
        Self: PartialEq + Clone,
    {
        let idx = Self::VARIANTS
            .iter()
            .position(|v| v == self)
            .unwrap_or_else(|| unreachable!());
        Self::VARIANTS[(idx + 1) % Self::VARIANTS.len()].clone()
    }
}

impl From<Theme> for ::iced::Theme {
    fn from(value: Theme) -> Self {
        macro_rules! themes {
            ($v:expr, $($theme:ident,)*) => {
                match $v {$(
                    Theme:: $theme => ::iced::Theme:: $theme,
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
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct State {
    /// Settings state.
    #[deref]
    #[deref_mut]
    pub settings: Settings,
    /// Path to config file.
    pub config: PathBuf,
}

async fn save(settings: Settings, path: PathBuf) -> Result<PathBuf, PathBuf> {
    match ::toml::to_string_pretty(&settings) {
        Ok(contents) => match ::tokio::fs::write(&path, contents).await {
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

impl State {
    /// Apply delta created from t to self.
    pub fn apply_from<T>(&mut self, t: T)
    where
        Delta: From<T>,
    {
        Delta::from(t).apply(self);
    }

    /// Update state by message.
    pub fn update(&mut self, message: Message, tx: &StatusSender) -> Task<Message> {
        match message {
            Message::Delta(delta) => {
                delta.apply(&mut self.settings);
            }
            Message::Save => {
                let tx = tx.clone();
                let settings = self.settings.clone();
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
    pub fn view(&self) -> Element<'_, Message> {
        w::col()
            .align_x(Alignment::Start)
            .width(Fill)
            .push(
                w::row()
                    .width(Fill)
                    .push(text("Settings").align_x(Alignment::Center).width(Fill))
                    .push(button("Save").padding(3).on_press(Message::Save)),
            )
            .push(horizontal_rule(2))
            .push(self.view_enums().map(Message::Delta).pipe(w::scroll))
            .push(horizontal_rule(2))
            .push(
                self.view_paths()
                    .push(horizontal_space().width(0))
                    .pipe(Element::from)
                    .map(Message::Delta)
                    .pipe(w::scroll),
            )
            .into()
    }
}
