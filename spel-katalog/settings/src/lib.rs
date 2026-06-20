//! Settings widgets.

use ::derive_more::{From, IsVariant};
use ::iced_core::{Alignment, Element, Length::Fill};
use ::iced_runtime::Task;
use ::iced_widget::{button, space, text};

use ::core::ops::{Deref, DerefMut};
use ::spel_katalog_common::{StatusSender, async_status, w};
use ::std::{collections::HashMap, path::PathBuf, sync::Arc};
use ::tap::Pipe;

pub use ::spel_katalog_settings_traits::*;

mod environment;
mod list;

#[doc(hidden)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/settings.rs"));
    pub use crate::environment::*;

    impl Settings {
        /// Get option by type
        pub fn get<T: super::AsIndex<Settings>>(&self) -> &T::Output {
            &self[T::as_idx()]
        }

        /// Get mutable option by type
        pub fn get_mut<T: super::AsIndex<Settings>>(&mut self) -> &mut T::Output {
            &mut self[T::as_idx()]
        }
    }
}
pub use generated::*;

/// A generic representation of current settings.
pub type Generic = HashMap<&'static str, String>;

impl<T> ::core::ops::Index<T> for Settings
where
    T: SettingsIndex<Settings>,
{
    type Output = T::Output;

    fn index(&self, index: T) -> &Self::Output {
        index.get(self)
    }
}

impl<T> ::core::ops::IndexMut<T> for Settings
where
    T: SettingsIndexMut<Settings>,
{
    fn index_mut(&mut self, index: T) -> &mut Self::Output {
        index.get_mut(self)
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
    pub settings: Arc<Settings>,
    /// Path to config file.
    pub config: PathBuf,
}

impl DerefMut for State {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.settings)
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
    match ::toml::to_string_pretty(&settings) {
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

impl State {
    /// Apply delta created from t to self.
    pub fn apply_from<T>(&mut self, t: T)
    where
        Delta: From<T>,
    {
        Delta::from(t).apply(self);
    }

    /// Get a snapshot of settings at time ov invocation.
    pub fn snapshot(&self) -> Arc<Settings> {
        Arc::clone(&self.settings)
    }

    /// Update state by message.
    pub fn update(&mut self, message: Message, tx: &StatusSender) -> Task<Message> {
        match message {
            Message::Delta(delta) => {
                delta.apply(self);
            }
            Message::Save => {
                let tx = tx.clone();
                let settings = (*self.settings).clone();
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
