//! View settings widgets.

mod list;

use ::core::ops::{Deref, DerefMut};
use ::std::path::PathBuf;

use ::derive_more::{From, IsVariant};
use ::iced_core::{Alignment, Element, Length::Fill};
use ::iced_runtime::Task;
use ::iced_widget::{button, space, text};
use ::spel_katalog_common::{StatusSender, async_status, w};
use ::spel_katalog_settings::{Delta, Settings, SettingsStore, save, view_enums, view_paths};
use ::tap::Pipe;

/// Convert settings theme to iced theme.
pub const fn conv_theme(value: ::spel_katalog_settings::Theme) -> ::iced_core::Theme {
    macro_rules! themes {
            ($v:expr, $($theme:ident,)*) => {
                match $v {$(
                    ::spel_katalog_settings::Theme:: $theme => ::iced_core::Theme:: $theme,
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

    /// Get element to display enum options.
    pub fn view_enums(
        &self,
    ) -> ::iced_core::Element<'_, Delta, ::iced_core::Theme, ::iced_widget::Renderer> {
        crate::list::enum_list(view_enums!(self, crate::list::enum_choice)).into()
    }

    /// Get element to display path options.
    pub fn view_paths(&self) -> ::iced_widget::Column<'_, Delta> {
        crate::list::path_list(view_paths!(self, crate::list::path_input))
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
