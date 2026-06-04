//! Info view for native game.

use ::core::iter;

use ::iced_core::{
    Alignment::{self, Center},
    Font,
    Length::Fill,
    alignment::Vertical,
};
use ::iced_runtime::Task;
use ::iced_widget::{
    self as widget,
    text_editor::{self, Binding},
};
use ::spel_katalog_common::{OrRequest, PushMaybe, w};
use ::spel_katalog_formats::{GameId, NativeGame};
use ::spel_katalog_native::Pool;
use ::tap::Pipe;
use ::uuid::Uuid;
use widget::text_editor::Content;

use crate::Element;

/// Message in use by native info view.
#[derive(Debug, Clone)]
pub enum Message {
    /// Update conf_view.
    ConfAction(widget::text_editor::Action),
    /// Remove thumbnail of game.
    RemoveThumb,
    /// Add thumbnail to game.
    AddThumb,
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
    pub fn update(
        &mut self,
        message: Message,
        _game_db: &Pool,
    ) -> Task<OrRequest<crate::Message, crate::Request>> {
        match message {
            Message::ConfAction(action) => {
                self.conf_view.perform(action);
                Task::none()
            }
            Message::RemoveThumb => todo!(),
            Message::AddThumb => todo!(),
        }
    }

    /// Draw game titlebar.
    pub fn titlebar<'a, M: 'a + From<crate::Message> + Clone>(
        &'a self,
        game: &'a ::spel_katalog_formats::Game,
        thumb: Option<&'a widget::image::Handle>,
        id: GameId,
        buttons: Element<'a, M>,
    ) -> Element<'a, M> {
        let Self {
            game: game_info, ..
        } = self;
        w::row()
            .align_y(Alignment::Start)
            .height(150)
            .push(
                thumb
                    .map_or_else(
                        || {
                            widget::button("Add Thumbnail")
                                .style(widget::button::success)
                                .padding(3)
                                .pipe(widget::container)
                                .center_x(150)
                                .center_y(150)
                                .style(widget::container::dark)
                                .pipe(Element::from)
                        },
                        |thumb| {
                            ::iced_aw::widget::ContextMenu::new(
                                widget::image(thumb).width(150).height(150),
                                || {
                                    ::spel_katalog_widget::ListMenu::new()
                                        .push(widget::text("Thumbnail"))
                                        .separator()
                                        .button("Replace", || Message::AddThumb)
                                        .button("Remove", || Message::RemoveThumb)
                                        .into()
                                },
                            )
                            .pipe(Element::from)
                        },
                    )
                    .map(|message| crate::Message::NativeInfo(message).into()),
            )
            .push_maybe(thumb.is_some().then(spel_katalog_widget::rule::vertical))
            .push(
                w::col()
                    .push(
                        w::row()
                            .push(
                                widget::text(game.name())
                                    .wrapping(widget::text::Wrapping::WordOrGlyph)
                                    .width(Fill)
                                    .align_x(Center),
                            )
                            .push(buttons)
                            .align_y(Vertical::Top),
                    )
                    .push(spel_katalog_widget::rule::horizontal())
                    .push(
                        w::row()
                            .push(widget::text("Runner").font(Font::MONOSPACE))
                            .push(spel_katalog_widget::rule::vertical())
                            .push_maybe(game_info.as_ref().map(|game_info| {
                                widget::value(&game_info.runner)
                                    .font(Font::MONOSPACE)
                                    .align_x(Alignment::Start)
                                    .width(Fill)
                            })),
                    )
                    .push(spel_katalog_widget::rule::horizontal())
                    .push(
                        w::row()
                            .push(widget::text("Uuid  ").font(Font::MONOSPACE))
                            .push(spel_katalog_widget::rule::vertical())
                            .push(
                                widget::value(id)
                                    .font(Font::MONOSPACE)
                                    .align_x(Alignment::Start)
                                    .width(Fill),
                            ),
                    ),
            )
            .into()
    }

    /// View native info.
    pub fn view(&self) -> Element<'_, OrRequest<Message, crate::Request>> {
        ::spel_katalog_widget::scrollable(widget::themer(
            Some(::iced_core::Theme::SolarizedDark),
            text_editor::TextEditor::new(&self.conf_view)
                .key_binding(|key_press| {
                    if let ::iced_core::keyboard::Key::Named(
                        ::iced_core::keyboard::key::Named::Tab,
                    ) = key_press.modified_key
                    {
                        Some(::iced_widget::text_editor::Binding::Sequence(
                            iter::repeat_with(|| ::iced_widget::text_editor::Binding::Insert(' '))
                                .take(4)
                                .collect(),
                        ))
                    } else {
                        Binding::from_key_press(key_press)
                    }
                })
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
