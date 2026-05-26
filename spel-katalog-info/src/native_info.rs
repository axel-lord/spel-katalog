//! Info view for native game.

use ::iced_core::{
    Alignment::{self, Center},
    Font,
    Length::Fill,
    Point,
    mouse::Button,
};
use ::iced_runtime::Task;
use ::iced_widget::{self as widget, text_editor};
use ::spel_katalog_common::{OrRequest, PushMaybe, w};
use ::spel_katalog_formats::{GameId, NativeGame};
use ::tap::Pipe;
use ::uuid::Uuid;
use widget::text_editor::Content;

use crate::Element;

/// Message in use by native info view.
#[derive(Debug, Clone)]
pub enum Message {
    /// Update conf_view.
    ConfAction(widget::text_editor::Action),
    /// Mouse moved in thumbnail.
    ThumbMouseMove(Point),
    /// Thumbnail was clicked.
    ThumbClicked(Button),
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
    /// Mouse position in thumbnail.
    thumb_mouse_pos: Option<Point>,
    /// Displa thumbnail context menu.
    display_thumb_menu: bool,
}

impl State {
    /// Construct new state.
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            game: None,
            conf_view: Content::new(),
            thumb_mouse_pos: None,
            display_thumb_menu: false,
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
            Message::ThumbMouseMove(point) => {
                self.thumb_mouse_pos = Some(point);
                Task::none()
            }
            Message::ThumbClicked(button) => match button {
                Button::Right => {
                    self.display_thumb_menu = !self.display_thumb_menu;
                    Task::none()
                }
                _ => {
                    self.display_thumb_menu = false;
                    Task::none()
                }
            },
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
            .push_maybe(thumb.map(|thumb| {
                let thumb = widget::image(thumb)
                    .width(150)
                    .height(150)
                    .pipe(widget::mouse_area)
                    .on_press(Message::ThumbClicked(Button::Left))
                    .on_right_press(Message::ThumbClicked(Button::Right))
                    .on_middle_press(Message::ThumbClicked(Button::Middle))
                    .pipe(Element::from)
                    .map(|message| crate::Message::NativeInfo(message).into());

                ::iced_aw::widget::ContextMenu::new(thumb, || {
                    widget::column(vec![widget::text("a").into(), widget::text("b").into()]).into()
                })
            }))
            .push_maybe(thumb.is_some().then(spel_katalog_widget::rule::vertical))
            .push(
                w::col()
                    .push(
                        w::row()
                            .push(widget::text(game.name()).width(Fill).align_x(Center))
                            .push(buttons),
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
