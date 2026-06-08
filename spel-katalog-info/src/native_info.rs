//! Info view for native game.

use ::std::{io::Cursor, sync::Arc};

use ::iced_core::{
    Alignment::{self, Center},
    Font,
    Length::{self, Fill},
    alignment::Vertical,
    keyboard::{Key, Modifiers, key},
};
use ::iced_runtime::Task;
use ::iced_widget::{
    self as widget,
    text_editor::{self, Action, Binding, Content, Edit},
};
use ::image::ImageFormat;
use ::rfd::AsyncFileDialog;
use ::smol::unblock;
use ::spel_katalog_common::{OrRequest, PushMaybe, w};
use ::spel_katalog_formats::{GameId, NativeGame};
use ::spel_katalog_native::Pool;
use ::tap::Pipe;
use ::uuid::Uuid;

use crate::Element;

/// Message in use by native info view.
#[derive(Debug, Clone)]
pub enum Message {
    /// Update conf_view.
    ConfAction(widget::text_editor::Action),
    /// Set content of text editor.
    SetConfig(Box<NativeGame>),
    /// Indent current line.
    Indent,
    /// Unindent current line.
    Unindent,
    /// Remove thumbnail of game.
    RemoveThumb,
    /// Add thumbnail to game.
    AddThumb,
    /// Save thumbnail of game.
    SaveThumb,
    /// Run game.
    Run,
    /// Run shell for game.
    Shell,
    /// Open game directory.
    Open,
    /// Discard changes.
    Discard,
    /// Save changes.
    Save,
    /// Copy selected text.
    Copy,
    /// Paste clipboard into text editor.
    Paste,
}

/// Request in use by native info view.
#[derive(Debug, Clone)]
pub enum Request {
    /// Remove thumbnail from games view.
    UndisplayThumbnail {
        /// Id of game to undisplay thumbnail for.
        id: GameId,
    },
    /// Add thumbnail to games view.
    DisplayThumbnail {
        /// Id of game to display thumbnail for.
        id: GameId,
        /// Thumbnail image.
        img: ::spel_katalog_formats::Image,
    },
    /// Run a game.
    RunGame(Box<NativeGame>),
    /// Run shell for a game.
    RunShell(Box<NativeGame>),
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
    /// Have any edits been made.
    has_edits: bool,
}

impl State {
    /// Construct new state.
    pub fn new(uuid: Uuid) -> Self {
        Self {
            uuid,
            game: None,
            conf_view: Content::new(),
            has_edits: false,
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
        self.has_edits = false;
    }

    /// Create a function that is ran with the parsed
    /// content of text editot if valid.
    pub fn with_content<T: 'static + Send>(
        &self,
        with: impl 'static + Send + FnOnce(NativeGame) -> Option<T>,
    ) -> Task<T> {
        let content = self.conf_view.text();
        Task::<Option<_>>::future(::smol::unblock(move || {
            let game = ::toml::from_str::<NativeGame>(&content)
                .map_err(|err| ::log::error!("content is not formatted correctly\n{err}"))
                .ok()?;
            with(game)
        }))
        .and_then(Task::done)
    }

    /// Update state using message.
    pub fn update(
        &mut self,
        message: Message,
        game_db: &Pool,
    ) -> Task<OrRequest<Message, Request>> {
        match message {
            Message::Run => self.with_content(|game| {
                Box::new(game)
                    .pipe(Request::RunGame)
                    .pipe(OrRequest::Request)
                    .pipe(Some)
            }),
            Message::Shell => self.with_content(|game| {
                Box::new(game)
                    .pipe(Request::RunShell)
                    .pipe(OrRequest::Request)
                    .pipe(Some)
            }),
            Message::Open => self.with_content(|game| {
                let Some(parent) = game.exe.parent() else {
                    ::log::error!("game executable {exe:?} has not parent", exe = game.exe);
                    return None;
                };

                if let Err(err) = ::open::that_detached(parent) {
                    ::log::error!("failed to open {parent:?}\n{err}");
                }

                None
            }),
            Message::Save => {
                let game_db = game_db.clone();
                let uuid = self.uuid;
                self.with_content(move |game| {
                    game_db
                        .insert_game(uuid)
                        .insert(&game)
                        .map_err(|err| {
                            ::log::error!("could not insert game {uuid} into database\n{err}")
                        })
                        .ok()?;

                    Box::new(game)
                        .pipe(Message::SetConfig)
                        .pipe(OrRequest::Message)
                        .pipe(Some)
                })
            }
            Message::Discard => {
                let game_db = game_db.clone();
                let uuid = self.uuid;
                Task::<Option<Message>>::future(::smol::unblock(move || {
                    game_db
                        .get_game(uuid)
                        .map_err(|err| ::log::error!("could not get game with uuid {uuid}\n{err}"))
                        .ok()
                        .map(Box::new)
                        .map(Message::SetConfig)
                }))
                .and_then(Task::done)
                .map(OrRequest::Message)
            }
            Message::SetConfig(config) => {
                self.set_config(*config);
                Task::none()
            }
            Message::ConfAction(action) => {
                if action.is_edit() {
                    self.has_edits = true;
                }
                self.conf_view.perform(action);
                Task::none()
            }
            Message::Indent => {
                self.conf_view.perform(Action::Edit(Edit::Indent));
                Task::none()
            }
            Message::Unindent => {
                self.conf_view.perform(Action::Edit(Edit::Unindent));
                Task::none()
            }
            Message::RemoveThumb => {
                let uuid = self.uuid;
                let game_db = game_db.clone();
                Task::future(async move {
                    if let Err(err) = unblock(move || game_db.remove_thumb(uuid)).await {
                        ::log::warn!("failed to remove thumbnail for {uuid} in database\n{err}");
                    }
                    GameId::Native(uuid)
                        .pipe(|id| Request::UndisplayThumbnail { id })
                        .pipe(OrRequest::Request)
                })
            }
            Message::SaveThumb => {
                let uuid = self.uuid;
                let game_db = game_db.clone();

                Task::future(async move {
                    let thumb = game_db
                        .get_thumb(uuid)
                        .map_err(|err| ::log::error!("could not get thumbnail for {uuid}\n{err}"))
                        .ok()?;

                    let mut buf = Vec::<u8>::new();
                    thumb
                        .write_to(Cursor::new(&mut buf), ImageFormat::Png)
                        .map_err(|err| {
                            ::log::error!("failed to encode thumbnail for {uuid}\n{err}")
                        })
                        .ok()?;

                    let dialog = AsyncFileDialog::new()
                        .add_filter("png", &["png"])
                        .save_file();

                    let Some(file) = dialog.await else {
                        ::log::warn!("no path chosen to save thumbnail of {uuid} to");
                        return None;
                    };

                    ::smol::fs::write(file.path(), &buf)
                        .await
                        .map_err(|err| {
                            ::log::error!(
                                "failed to write thumbnail of {uuid} to {path:?}\n{err}",
                                path = file.path()
                            )
                        })
                        .ok()?;

                    Some(())
                })
                .and_then(|_| Task::none())
            }
            Message::AddThumb => {
                let uuid = self.uuid;
                let game_db = game_db.clone();

                Task::future(async move {
                    let dialog = AsyncFileDialog::new()
                        .set_title("Set Thumbnail")
                        .add_filter("png", &["png"])
                        .pick_file();

                    let Some(file) = dialog.await else {
                        ::log::info!("no thumbnail chosen for {uuid}");
                        return None;
                    };

                    let content = ::smol::fs::read(file.path())
                        .await
                        .map_err(|err| {
                            ::log::error!("could not read {path:?}\n{err}", path = file.path())
                        })
                        .ok()?;

                    let image = ::image::load_from_memory(&content)
                        .map_err(|err| {
                            ::log::error!("could not load {path:?}\n{err}", path = file.path())
                        })
                        .ok()?;

                    game_db
                        .insert_thumb(uuid)
                        .insert(&image)
                        .map_err(|err| {
                            ::log::error!(
                                "could not insert thumbnail {path:?} into database\n{err}",
                                path = file.path()
                            )
                        })
                        .ok()?;

                    Request::DisplayThumbnail {
                        id: GameId::Native(uuid),
                        img: ::spel_katalog_native::thumbnail(image),
                    }
                    .pipe(OrRequest::Request)
                    .pipe(Some)
                })
                .and_then(Task::done)
            }
            Message::Paste => ::iced_runtime::clipboard::read().and_then(|content| {
                Arc::new(content)
                    .pipe(Edit::Paste)
                    .pipe(Action::Edit)
                    .pipe(Message::ConfAction)
                    .pipe(OrRequest::Message)
                    .pipe(Task::done)
            }),
            Message::Copy => self
                .conf_view
                .selection()
                .map(::iced_runtime::clipboard::write)
                .unwrap_or_else(Task::none),
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
        const DIM: u32 = 200;
        let Self {
            game: game_info, ..
        } = self;
        let name = game_info
            .as_ref()
            .map(|game| game.name.as_str())
            .unwrap_or(game.name());
        w::col()
            .push(
                w::row()
                    .push(
                        widget::text(name)
                            .wrapping(widget::text::Wrapping::WordOrGlyph)
                            .width(Fill)
                            .align_x(Center),
                    )
                    .push(buttons)
                    .align_y(Vertical::Center),
            )
            .push(spel_katalog_widget::rule::horizontal())
            .push(
                w::row()
                    .align_y(Alignment::Start)
                    .height(DIM)
                    .push(
                        thumb
                            .map_or_else(
                                || {
                                    widget::button("Add Thumbnail")
                                        .on_press_with(|| Message::AddThumb)
                                        .style(widget::button::success)
                                        .padding(3)
                                        .pipe(widget::container)
                                        .center_x(DIM)
                                        .center_y(DIM)
                                        .style(widget::container::dark)
                                        .pipe(Element::from)
                                },
                                |thumb| {
                                    ::iced_aw::widget::ContextMenu::new(
                                        widget::image(thumb).width(DIM).height(DIM),
                                        || {
                                            ::spel_katalog_widget::ListMenu::new()
                                                .push(widget::text("Thumbnail"))
                                                .separator()
                                                .button("Replace", || Message::AddThumb)
                                                .button("Remove", || Message::RemoveThumb)
                                                .button("Save As", || Message::SaveThumb)
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
                    ),
            )
            .into()
    }

    /// View native info.
    pub fn view(&self) -> Element<'_, OrRequest<Message, crate::Request>> {
        widget::Column::new()
            .spacing(3)
            .push(
                widget::Row::new()
                    .spacing(3)
                    .push(
                        widget::button("Run")
                            .on_press_with(|| Message::Run)
                            .padding(3)
                            .style(widget::button::success),
                    )
                    .push(
                        widget::button("Shell")
                            .on_press_with(|| Message::Shell)
                            .padding(3),
                    )
                    .push(widget::space().width(Length::Fill))
                    .push(
                        widget::button("Open")
                            .on_press_with(|| Message::Open)
                            .padding(3),
                    )
                    .push(
                        widget::button("Discard")
                            .on_press_maybe(self.has_edits.then_some(Message::Discard))
                            .padding(3)
                            .style(widget::button::danger),
                    )
                    .push(
                        widget::button("Save")
                            .on_press_maybe(self.has_edits.then_some(Message::Save))
                            .padding(3)
                            .style(widget::button::success),
                    )
                    .pipe(Element::from)
                    .map(OrRequest::Message),
            )
            .push(::spel_katalog_widget::scrollable(
                ::iced_aw::widget::ContextMenu::new(
                    widget::themer(
                        Some(::iced_core::Theme::SolarizedDark),
                        text_editor::TextEditor::new(&self.conf_view)
                            .key_binding(|event| {
                                if let Key::Named(key::Named::Tab) = event.modified_key {
                                    if event.modifiers == Modifiers::empty() {
                                        Message::Indent
                                            .pipe(OrRequest::Message)
                                            .pipe(Binding::Custom)
                                            .pipe(Some)
                                    } else if event.modifiers == Modifiers::SHIFT {
                                        Message::Unindent
                                            .pipe(OrRequest::Message)
                                            .pipe(Binding::Custom)
                                            .pipe(Some)
                                    } else {
                                        Binding::from_key_press(event)
                                    }
                                } else {
                                    Binding::from_key_press(event)
                                }
                            })
                            .highlight_with::<::iced_highlighter::Highlighter>(
                                ::iced_highlighter::Settings {
                                    theme: ::iced_highlighter::Theme::SolarizedDark,
                                    token: "toml".to_owned(),
                                },
                                |h, _| h.to_format(),
                            )
                            .on_action(|action| {
                                action.pipe(Message::ConfAction).pipe(OrRequest::Message)
                            })
                            .padding(6),
                    ),
                    || {
                        ::spel_katalog_widget::ListMenu::new()
                            .push(widget::text("Config"))
                            .separator()
                            .button("Copy", || OrRequest::Message(Message::Copy))
                            .button("Paste", || OrRequest::Message(Message::Paste))
                            .into()
                    },
                ),
            ))
            .into()
    }
}
