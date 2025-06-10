//! Settings widgets.
#![allow(missing_docs)]

use ::derive_more::{Deref, DerefMut, From, IsVariant};
use ::iced::{
    Alignment, Element,
    Length::Fill,
    Task,
    widget::{button, horizontal_rule, text},
};

use ::spel_katalog_common::{StatusSender, async_status, w};
use ::std::path::PathBuf;
use ::tap::Pipe;

mod list;

#[doc(hidden)]
mod generated {
    #![allow(missing_docs)]

    pub static HOME: ::spel_katalog_common::lazy::Lazy =
        ::spel_katalog_common::lazy::Lazy::new(|| {
            String::from(match &::std::env::var("HOME") {
                Ok(home) => home.as_str().trim_end_matches('/'),
                Err(err) => {
                    ::log::warn!("could not get home directory, {err}");
                    "/opt"
                }
            })
        });

    include!(concat!(env!("OUT_DIR"), "/settings.rs"));
}
pub use generated::*;

/// Trait to provide a default string representation of a type.
pub trait DefaultStr {
    /// Get the default string representation of self.
    fn default_str() -> &'static str;
}

/// Trait to provide settings titles.
pub trait Title {
    /// Title to use for setting.
    fn title() -> &'static str;
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
}

impl From<Theme> for ::iced::Theme {
    fn from(value: Theme) -> Self {
        match value {
            Theme::Light => ::iced::Theme::Light,
            Theme::Dark => ::iced::Theme::Dark,
        }
    }
}

#[derive(Debug, IsVariant, Clone, From)]
pub enum Message {
    Delta(Delta),
    Save,
}

#[derive(Debug, Clone, Deref, DerefMut)]
pub struct State {
    #[deref]
    #[deref_mut]
    pub settings: Settings,
    pub config: Option<PathBuf>,
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
    pub fn update(&mut self, message: Message, tx: &StatusSender) -> Task<Message> {
        match message {
            Message::Delta(delta) => {
                delta.apply(&mut self.settings);
            }
            Message::Save => {
                if let Some(path) = &self.config {
                    let tx = tx.clone();
                    let settings = self.settings.clone();
                    let path = path.to_path_buf();
                    return Task::future(async move {
                        match save(settings, path).await {
                            Ok(path) => async_status!(tx, "saved settings to {path:?}").await,
                            Err(path) => {
                                async_status!(tx, "could not save settings to {path:?}").await
                            }
                        }
                    })
                    .then(|_| Task::none());
                }
            }
        };
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        w::col()
            .align_x(Alignment::Start)
            .width(Fill)
            .push(
                w::row()
                    .width(Fill)
                    .push(text("Settings").align_x(Alignment::Center).width(Fill))
                    .push(
                        button("Save")
                            .padding(3)
                            .on_press_maybe(self.config.is_some().then_some(Message::Save)),
                    ),
            )
            .push(horizontal_rule(2))
            .push(self.view_enums().map(Message::Delta).pipe(w::scroll))
            .push(horizontal_rule(2))
            .push(self.view_paths().map(Message::Delta).pipe(w::scroll))
            .into()
    }
}
