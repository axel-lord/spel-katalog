//! Info view for native game.

use ::iced_runtime::Task;
use ::iced_widget::{self as widget, text_editor};
use ::spel_katalog_common::OrRequest;
use ::spel_katalog_formats::NativeGame;
use ::tap::Pipe;
use ::uuid::Uuid;
use widget::text_editor::Content;

use crate::Element;

/// Message in use by native info view.
#[derive(Debug, Clone)]
pub enum Message {
    /// Update conf_view.
    ConfAction(widget::text_editor::Action),
}

/// State of native game display.
#[derive(Debug)]
pub struct State {
    /// Game uuid.
    pub uuid: Uuid,
    /// Game config.
    game: Option<NativeGame>,
    /// Config view.
    conf_view: Content,
}

impl State {
    /// Construct new state.
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            game: None,
            conf_view: Content::new(),
        }
    }

    /// Set game config in use.
    pub fn set_config(&mut self, config: NativeGame) {
        match ::toml::to_string_pretty(&config) {
            Ok(text) => {
                crate::set_content(&mut self.conf_view, text);
            }
            Err(err) => ::log::warn!(
                "could not serialize game config for {uuid}\n{err}",
                uuid = self.uuid
            ),
        }
        self.game = Some(config);
    }

    /// Update state using message.
    pub fn update(&mut self, message: Message) -> Task<OrRequest<crate::Message, crate::Request>> {
        match message {
            Message::ConfAction(action) => {
                self.conf_view.perform(action);
                Task::none()
            }
        }
    }

    /// View native info.
    pub fn view(&self) -> Element<'_, OrRequest<Message, crate::Request>> {
        ::spel_katalog_widget::scrollable(widget::themer(
            Some(::iced_core::Theme::SolarizedDark),
            text_editor::TextEditor::new(&self.conf_view)
                .highlight_with::<::iced_highlighter::Highlighter>(
                    ::iced_highlighter::Settings {
                        theme: ::iced_highlighter::Theme::SolarizedDark,
                        token: "toml".to_owned(),
                    },
                    |h, _| h.to_format(),
                )
                .on_action(|action| action.pipe(Message::ConfAction).pipe(OrRequest::Message))
                .padding(6),
        ))
        .into()
    }
}
