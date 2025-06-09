//! Settings widgets.
#![allow(missing_docs)]

use ::derive_more::{Deref, DerefMut, From, IsVariant};
use ::iced::{
    Alignment, Element,
    Length::Fill,
    Task,
    widget::{button, horizontal_rule, text},
};

use ::spel_katalog_common::{OrStatus, status, w};
use ::std::path::PathBuf;
use ::tap::Pipe;

use crate::list::{pl, pl_list, ti, ti_list};

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
    pub fn update(&mut self, message: Message) -> Task<OrStatus<Message>> {
        match message {
            Message::Delta(delta) => {
                delta.apply(&mut self.settings);
            }
            Message::Save => {
                if let Some(path) = &self.config {
                    return Task::future(save(self.settings.clone(), path.to_path_buf())).then(
                        |result| {
                            Task::done(match result {
                                Ok(path) => status!("saved settings to {path:?}"),
                                Err(path) => status!("could not save settings to {path:?}"),
                            })
                        },
                    );
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
            .push(
                pl_list([
                    pl(self.settings.theme),
                    pl(self.settings.show),
                    pl(self.settings.filter_mode),
                    pl(self.settings.sort_by),
                    pl(self.settings.sort_dir),
                    pl(self.settings.network),
                ])
                .pipe(Element::from)
                .map(Message::Delta),
            )
            .push(horizontal_rule(2))
            .push(
                ti_list([
                    ti(&self.settings.lutris_exe),
                    ti(&self.settings.firejail_exe),
                    ti(&self.settings.coverart_dir),
                    ti(&self.settings.lutris_db),
                    ti(&self.settings.yml_dir),
                ])
                .pipe(Element::from)
                .map(Message::Delta)
                .pipe(w::scroll),
            )
            .into()
    }
}
