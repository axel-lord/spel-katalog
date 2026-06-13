//! Config editor.

use ::std::sync::Arc;

use ::iced_core::{
    Element,
    Length::{self, Fill},
    keyboard::{Modifiers, key},
    text::Wrapping,
};
use ::iced_runtime::Task;
use ::iced_widget::{
    self as widget,
    text_editor::{self, Binding},
};
use ::spel_katalog_common::{IntoOrRequest, OrRequest, w};
use ::spel_katalog_formats::NativeGame;
use ::spel_katalog_settings::Settings;
use ::tap::Pipe;

/// Message used by config editor.
#[derive(Debug, Clone)]
pub enum Message {
    /// Perform an editor action.
    Action(text_editor::Action),
    /// Undo action.
    Undo,
    /// Redo undone action.
    Redo,
    /// Copy selected content to clipboard.
    Copy,
    /// Paste clipboard.
    Paste,
}

/// Installer window for a game.
#[derive(Debug, Clone)]
pub struct Editor {
    /// Editor content.
    content: text_editor::Content,
    /// History of content.
    history: Vec<String>,
    /// Future of content.
    future: Vec<String>,
}

impl Editor {
    /// Construct a new editor.
    pub fn new(game: &NativeGame) -> Option<Self> {
        let content = ::toml::to_string_pretty(game)
            .map_err(|err| ::log::error!("could not serialize game\n{game:#?}\n{err}"))
            .ok()?;

        let mut editor = Self {
            content: text_editor::Content::new(),
            history: Vec::new(),
            future: Vec::new(),
        };

        editor.set_content(content);

        Some(editor)
    }

    /// Set content of text editor.
    fn set_content(&mut self, content: String) {
        w::set_text_editor_content(&mut self.content, content);
    }

    /// Update application state using message.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Action(action) => {
                if action.is_edit() {
                    self.history.push(self.content.text());
                    self.future.clear();
                }
                self.content.perform(action);
                Task::none()
            }
            Message::Undo => {
                if let Some(content) = self.history.pop() {
                    self.future.push(self.content.text());
                    self.set_content(content);
                }
                Task::none()
            }
            Message::Redo => {
                if let Some(content) = self.future.pop() {
                    self.history.push(self.content.text());
                    self.set_content(content);
                }
                Task::none()
            }
            Message::Copy => self
                .content
                .selection()
                .map(::iced_runtime::clipboard::write)
                .unwrap_or_else(Task::none),
            Message::Paste => ::iced_runtime::clipboard::read().and_then(|content| {
                content
                    .pipe(Arc::new)
                    .pipe(text_editor::Edit::Paste)
                    .pipe(text_editor::Action::Edit)
                    .pipe(Message::Action)
                    .pipe(Task::done)
            }),
        }
    }

    /// View editor.
    fn view_text_editor(
        &self,
        settings: &Settings,
    ) -> Element<'_, Message, ::iced_core::Theme, widget::Renderer> {
        ::iced_aw::ContextMenu::new(
            widget::text_editor(&self.content)
                .on_action(Message::Action)
                .height(Fill)
                .wrapping(Wrapping::Glyph)
                .key_binding(|event| match event.modified_key.as_ref() {
                    ::iced_core::keyboard::Key::Named(named) => match named {
                        key::Named::Tab if event.modifiers == Modifiers::empty() => {
                            text_editor::Edit::Indent
                                .pipe(text_editor::Action::Edit)
                                .pipe(Message::Action)
                                .pipe(Binding::Custom)
                                .pipe(Some)
                        }
                        key::Named::Tab if event.modifiers == Modifiers::SHIFT => {
                            text_editor::Edit::Unindent
                                .pipe(text_editor::Action::Edit)
                                .pipe(Message::Action)
                                .pipe(Binding::Custom)
                                .pipe(Some)
                        }
                        _ => Binding::from_key_press(event),
                    },
                    ::iced_core::keyboard::Key::Character(chr) => match chr {
                        "z" if event.modifiers == Modifiers::CTRL => {
                            Message::Undo.pipe(Binding::Custom).pipe(Some)
                        }
                        "y" if event.modifiers == Modifiers::CTRL => {
                            Message::Redo.pipe(Binding::Custom).pipe(Some)
                        }
                        _ => Binding::from_key_press(event),
                    },
                    ::iced_core::keyboard::Key::Unidentified => Binding::from_key_press(event),
                })
                .highlight_with::<::iced_highlighter::Highlighter>(
                    ::iced_highlighter::Settings {
                        theme: match settings.get::<::spel_katalog_settings::Theme>() {
                            ::spel_katalog_settings::Theme::SolarizedDark => {
                                ::iced_highlighter::Theme::SolarizedDark
                            }
                            theme
                                if ::iced_core::Theme::from(*theme).extended_palette().is_dark =>
                            {
                                ::iced_highlighter::Theme::Base16Mocha
                            }
                            _ => ::iced_highlighter::Theme::InspiredGitHub,
                        },
                        token: "toml".to_owned(),
                    },
                    |h, _| h.to_format(),
                ),
            || {
                ::spel_katalog_widget::ListMenu::new()
                    .push(widget::text("Config"))
                    .separator()
                    .button("Copy", || Message::Copy)
                    .button("Paste", || Message::Paste)
                    .separator()
                    .button_if(!self.history.is_empty(), "Undo", || Message::Undo)
                    .button_if(!self.future.is_empty(), "Redo", || Message::Redo)
                    .into()
            },
        )
        .into()
    }

    /// View button row.
    fn view_buttons(&self) -> widget::Row<'_, super::Message> {
        widget::Row::new()
            .spacing(3)
            .push(::iced_aw::widget::ContextMenu::new(
                widget::button("Open")
                    .padding(3)
                    .on_press(super::Message::SelectDir(None)),
                || {
                    ::spel_katalog_widget::ListMenu::new()
                        .push(widget::text("Open"))
                        .separator()
                        .button("File", || super::Message::SelectFile)
                        .button("Folder", || super::Message::SelectDir(None))
                        .into()
                },
            ))
            .push(widget::space().width(Length::Fill))
    }

    /// View config editor (text editor + button row)
    fn view_config_editor(
        &self,
        settings: &Settings,
    ) -> widget::Container<'_, OrRequest<super::Message, super::Request>> {
        widget::container(
            widget::Column::new()
                .spacing(3)
                .push(
                    self.view_buttons()
                        .pipe(Element::from)
                        .map(OrRequest::Message),
                )
                .push(
                    self.view_text_editor(settings)
                        .pipe(Element::from)
                        .map(super::Message::Editor)
                        .map(OrRequest::Message),
                )
                .push(
                    widget::Row::new()
                        .spacing(3)
                        .push(widget::space().width(Length::Fill))
                        .push(
                            widget::button("Cancel")
                                .padding(3)
                                .style(widget::button::danger)
                                .on_press(super::Message::UnsetEditor.into_message()),
                        )
                        .push(
                            widget::button("Install")
                                .padding(3)
                                .style(widget::button::success),
                        ),
                ),
        )
        .padding(6)
    }

    /// View installer state.
    pub fn view(
        &self,
        settings: &Settings,
    ) -> Element<'_, OrRequest<super::Message, super::Request>, ::iced_core::Theme, widget::Renderer>
    {
        self.view_config_editor(settings).into()
    }
}
