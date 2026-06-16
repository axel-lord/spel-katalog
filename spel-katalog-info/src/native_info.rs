//! Info view for native game.

use ::core::ops::Not;
use ::std::{borrow::Cow, io::Cursor, sync::Arc};

use ::derive_more::From;
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
use ::spel_katalog_common::{IntoOrRequest, OrRequest, in_place::PushMaybe as _, w};
use ::spel_katalog_formats::{GameId, NativeGame};
use ::spel_katalog_native::Pool;
use ::spel_katalog_settings::{CompToolsDir, ThmubnailSource};
use ::spel_katalog_widget::monospace;
use ::tap::{Pipe, TapOptional};
use ::uuid::Uuid;
use spel_katalog_settings::Settings;

use crate::{Element, native_table::InfoTable};

/// Short message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuickMessage {
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
    /// Initialize prefix.
    Init,
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
    /// Undo last edit action.
    Undo,
    /// Redo last edit action.
    Redo,
    /// Add a bind to game config.
    AddBind,
    /// Add a specific compatability tool.
    AddCompTool,
}

/// Message in use by native info view.
#[derive(Debug, Clone, From)]
pub enum Message {
    /// Update conf_view.
    ConfAction(widget::text_editor::Action),
    /// Set content of text editor.
    SetConfig(Box<NativeGame>),
    /// Update content of text editor.
    UpdateConfig(Box<NativeGame>),
    /// Quick message.
    #[from]
    Quick(QuickMessage),
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
    /// Init prefix for a game.
    RunInit(Box<NativeGame>),
}

/// State of native game display.
#[derive(Debug)]
pub struct State {
    /// Game uuid.
    pub uuid: Uuid,
    /// Config view.
    conf_view: Content,
    /// Old config versions.
    history: Vec<String>,
    /// Future config versions.
    future: Vec<String>,
}

impl State {
    /// Construct new state.
    pub fn new(uuid: Uuid, game: NativeGame) -> Self {
        let mut state = Self {
            uuid,
            conf_view: Content::new(),
            history: Vec::new(),
            future: Vec::new(),
        };

        if let Ok(content) = ::toml::to_string_pretty(&game) {
            state.set_content(content);
        };

        state
    }

    /// Set game config in use.
    pub fn set_config(&mut self, config: NativeGame, flush_history: bool) {
        match ::toml::to_string_pretty(&config) {
            Ok(text) => {
                self.future.clear();
                if !flush_history {
                    self.history.push(self.conf_view.text());
                }

                self.set_content(text);

                if flush_history {
                    self.history.clear();
                }
            }
            Err(err) => ::log::warn!(
                "could not serialize game config for {uuid}\n{err}",
                uuid = self.uuid
            ),
        }
    }

    /// Set text content of config editor.
    pub fn set_content(&mut self, content: String) {
        w::set_text_editor_content(&mut self.conf_view, content);
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

    /// I content can be parsed create create a task with it.
    pub fn get_content(&self) -> Task<NativeGame> {
        self.with_content(Some)
    }

    /// Update state using message.
    pub fn update(
        &mut self,
        message: Message,
        game_db: &Pool,
        settings: &Settings,
    ) -> Task<OrRequest<Message, Request>> {
        match message {
            Message::SetConfig(config) => {
                self.set_config(*config, true);
                Task::none()
            }
            Message::UpdateConfig(config) => {
                self.set_config(*config, false);
                Task::none()
            }
            Message::ConfAction(action) => {
                if action.is_edit() {
                    self.history.push(self.conf_view.text());
                    self.future.clear();
                }
                self.conf_view.perform(action);
                Task::none()
            }
            Message::Quick(message) => match message {
                QuickMessage::AddCompTool => {
                    let game_db = game_db.clone();
                    let uuid = self.uuid;
                    let comp_tool_dir = settings.get::<CompToolsDir>().to_path_buf();

                    Task::<Option<_>>::future(async move {
                        let dialog = AsyncFileDialog::new()
                            .set_directory(&comp_tool_dir)
                            .pick_folder()
                            .await
                            .tap_none(|| ::log::info!("no comp tool chosen"))?;

                        let mut game = game_db
                            .get_game(uuid)
                            .inspect_err(|err| {
                                ::log::error!("failed to get game with uuid {uuid}\n{err}")
                            })
                            .ok()?;

                        game.env.insert(
                            "PROTONPATH".to_owned(),
                            dialog.path().as_os_str().to_string_lossy().into(),
                        );

                        Box::new(game)
                            .pipe(Message::UpdateConfig)
                            .into_message()
                            .pipe(Some)
                    })
                    .and_then(Task::done)
                }
                QuickMessage::AddBind => self
                    .get_content()
                    .then(|mut game| {
                        Task::<Option<_>>::future(async move {
                            game.bind.push(
                                game.exe
                                    .parent()
                                    .map_or_else(AsyncFileDialog::new, |parent| {
                                        AsyncFileDialog::new().set_directory(parent)
                                    })
                                    .pick_folder()
                                    .await
                                    .tap_none(|| ::log::warn!("no folder chosen"))?
                                    .path()
                                    .to_path_buf()
                                    .pipe(::spel_katalog_formats::Bind::mirrored),
                            );

                            Box::new(game)
                                .pipe(Message::UpdateConfig)
                                .pipe(OrRequest::Message)
                                .pipe(Some)
                        })
                    })
                    .and_then(Task::done),
                QuickMessage::Run => self.with_content(|game| {
                    Box::new(game)
                        .pipe(Request::RunGame)
                        .pipe(OrRequest::Request)
                        .pipe(Some)
                }),
                QuickMessage::Shell => self.with_content(|game| {
                    Box::new(game)
                        .pipe(Request::RunShell)
                        .pipe(OrRequest::Request)
                        .pipe(Some)
                }),
                QuickMessage::Init => self.with_content(|game| {
                    Box::new(game)
                        .pipe(Request::RunInit)
                        .pipe(OrRequest::Request)
                        .pipe(Some)
                }),
                QuickMessage::Open => self.with_content(|game| {
                    let parent = game.exe.parent().tap_none(|| {
                        ::log::error!("game executable {exe:?} has not parent", exe = game.exe)
                    })?;

                    if let Err(err) = ::open::that_detached(parent) {
                        ::log::error!("failed to open {parent:?}\n{err}");
                    }

                    None
                }),
                QuickMessage::Save => {
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
                QuickMessage::Discard => {
                    let game_db = game_db.clone();
                    let uuid = self.uuid;
                    Task::<Option<Message>>::future(::smol::unblock(move || {
                        game_db
                            .get_game(uuid)
                            .map_err(|err| {
                                ::log::error!("could not get game with uuid {uuid}\n{err}")
                            })
                            .ok()
                            .map(Box::new)
                            .map(Message::SetConfig)
                    }))
                    .and_then(Task::done)
                    .map(OrRequest::Message)
                }
                QuickMessage::Indent => {
                    self.conf_view.perform(Action::Edit(Edit::Indent));
                    Task::none()
                }
                QuickMessage::Unindent => {
                    self.conf_view.perform(Action::Edit(Edit::Unindent));
                    Task::none()
                }
                QuickMessage::RemoveThumb => {
                    let uuid = self.uuid;
                    let game_db = game_db.clone();
                    Task::future(async move {
                        if let Err(err) = unblock(move || game_db.remove_thumb(uuid)).await {
                            ::log::warn!(
                                "failed to remove thumbnail for {uuid} in database\n{err}"
                            );
                        }
                        GameId::Native(uuid)
                            .pipe(|id| Request::UndisplayThumbnail { id })
                            .pipe(OrRequest::Request)
                    })
                }
                QuickMessage::SaveThumb => {
                    let uuid = self.uuid;
                    let game_db = game_db.clone();

                    Task::future(async move {
                        let mut buf = Vec::<u8>::new();
                        game_db
                            .get_thumb(uuid)
                            .map_err(|err| {
                                ::log::error!("could not get thumbnail for {uuid}\n{err}")
                            })
                            .ok()?
                            .write_to(Cursor::new(&mut buf), ImageFormat::Png)
                            .map_err(|err| {
                                ::log::error!("failed to encode thumbnail for {uuid}\n{err}")
                            })
                            .ok()?;

                        let file = AsyncFileDialog::new()
                            .add_filter("png", &["png"])
                            .save_file()
                            .await
                            .tap_none(|| {
                                ::log::warn!("no path chosen to save thumbnail of {uuid} to")
                            })?;

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
                QuickMessage::AddThumb => {
                    let uuid = self.uuid;
                    let game_db = game_db.clone();
                    let location = settings.get::<ThmubnailSource>().to_path_buf();

                    Task::future(async move {
                        let file = AsyncFileDialog::new()
                            .set_title("Set Thumbnail")
                            .set_directory(location)
                            .add_filter(
                                "image",
                                &[
                                    "png", "jpg", "jpeg", "avif", "webp", "bmp", "tga", "tiff",
                                    "gif", "ico", "pnm", "ff", "exr",
                                ],
                            )
                            .pick_file()
                            .await
                            .tap_none(|| ::log::info!("no thumbnail chosen for {uuid}"))?;

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

                        let thumb = ::spel_katalog_native::make_square_thumbnail(Cow::Owned(image))
                            .tap_none(|| ::log::warn!("could not make thubmnail square"))?;

                        game_db
                            .insert_thumb(uuid)
                            .insert(&thumb)
                            .map_err(|err| {
                                ::log::error!(
                                    "could not insert thumbnail {path:?} into database\n{err}",
                                    path = file.path()
                                )
                            })
                            .ok()?;

                        Request::DisplayThumbnail {
                            id: GameId::Native(uuid),
                            img: ::spel_katalog_native::thumbnail(thumb.into_owned()),
                        }
                        .pipe(OrRequest::Request)
                        .pipe(Some)
                    })
                    .and_then(Task::done)
                }
                QuickMessage::Paste => ::iced_runtime::clipboard::read().and_then(|content| {
                    content
                        .pipe(Arc::new)
                        .pipe(Edit::Paste)
                        .pipe(Action::Edit)
                        .pipe(Message::ConfAction)
                        .pipe(OrRequest::Message)
                        .pipe(Task::done)
                }),
                QuickMessage::Copy => self
                    .conf_view
                    .selection()
                    .map(::iced_runtime::clipboard::write)
                    .unwrap_or_else(Task::none),
                QuickMessage::Undo => {
                    if let Some(content) = self.history.pop() {
                        self.future.push(self.conf_view.text());
                        self.set_content(content);
                    }
                    Task::none()
                }
                QuickMessage::Redo => {
                    if let Some(content) = self.future.pop() {
                        self.history.push(self.conf_view.text());
                        self.set_content(content);
                    }
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
        shadows: Option<GameId>,
        buttons: Element<'a, M>,
    ) -> Element<'a, M> {
        const DIM: u32 = 200;
        let name = game.name();
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
                                        .on_press_with(|| QuickMessage::AddThumb)
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
                                                .button("Replace", || QuickMessage::AddThumb)
                                                .button("Remove", || QuickMessage::RemoveThumb)
                                                .button("Save As", || QuickMessage::SaveThumb)
                                                .into()
                                        },
                                    )
                                    .pipe(Element::from)
                                },
                            )
                            .map(|message| {
                                crate::Message::NativeInfo(Message::Quick(message)).into()
                            }),
                    )
                    .push_maybe(thumb.is_some().then(spel_katalog_widget::rule::vertical))
                    .push(
                        monospace(InfoTable { id, shadows }.get_table().to_string())
                            .wrapping(::iced_core::text::Wrapping::Glyph),
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
                            .on_press_with(|| QuickMessage::Run)
                            .padding(3)
                            .style(widget::button::success),
                    )
                    .push(
                        widget::button("Shell")
                            .on_press_with(|| QuickMessage::Shell)
                            .padding(3),
                    )
                    .push(
                        widget::button("Init")
                            .on_press_with(|| QuickMessage::Init)
                            .padding(3),
                    )
                    .push(widget::space().width(Length::Fill))
                    .push(
                        widget::button("Open")
                            .on_press_with(|| QuickMessage::Open)
                            .padding(3),
                    )
                    .push(
                        widget::button("Discard")
                            .on_press_maybe(
                                self.history
                                    .is_empty()
                                    .not()
                                    .then_some(QuickMessage::Discard),
                            )
                            .padding(3)
                            .style(widget::button::danger),
                    )
                    .push(
                        widget::button("Save")
                            .on_press_maybe(
                                self.history.is_empty().not().then_some(QuickMessage::Save),
                            )
                            .padding(3)
                            .style(widget::button::success),
                    )
                    .pipe(Element::from)
                    .map(Message::Quick)
                    .map(OrRequest::Message),
            )
            .push(::spel_katalog_widget::scrollable(
                ::iced_aw::widget::ContextMenu::new(
                    widget::themer(
                        Some(::iced_core::Theme::SolarizedDark),
                        text_editor::TextEditor::new(&self.conf_view)
                            .font(Font::MONOSPACE)
                            .wrapping(::iced_core::text::Wrapping::Glyph)
                            .key_binding(|event| {
                                if let Key::Named(named) = event.modified_key {
                                    match named {
                                        key::Named::Tab
                                            if event.modifiers == Modifiers::empty() =>
                                        {
                                            QuickMessage::Indent
                                                .pipe(Message::Quick)
                                                .pipe(OrRequest::Message)
                                                .pipe(Binding::Custom)
                                                .pipe(Some)
                                        }

                                        key::Named::Tab if event.modifiers == Modifiers::SHIFT => {
                                            QuickMessage::Unindent
                                                .pipe(Message::Quick)
                                                .pipe(OrRequest::Message)
                                                .pipe(Binding::Custom)
                                                .pipe(Some)
                                        }
                                        _ => Binding::from_key_press(event),
                                    }
                                } else if let Key::Character(chr) = event.modified_key.as_ref() {
                                    match chr {
                                        "z" if event.modifiers == Modifiers::CTRL => {
                                            QuickMessage::Undo
                                                .pipe(Message::Quick)
                                                .pipe(OrRequest::Message)
                                                .pipe(Binding::Custom)
                                                .pipe(Some)
                                        }
                                        "y" if event.modifiers == Modifiers::CTRL => {
                                            QuickMessage::Redo
                                                .pipe(Message::Quick)
                                                .pipe(OrRequest::Message)
                                                .pipe(Binding::Custom)
                                                .pipe(Some)
                                        }
                                        _ => Binding::from_key_press(event),
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
                            .min_height(200)
                            .padding(6),
                    ),
                    || {
                        ::spel_katalog_widget::ListMenu::new()
                            .push(widget::text("Config"))
                            .separator()
                            .button_if(self.conf_view.selection().is_some(), "Copy", || {
                                QuickMessage::Copy
                            })
                            .button("Paste", || QuickMessage::Paste)
                            .separator()
                            .button_if(!self.history.is_empty(), "Undo", || QuickMessage::Undo)
                            .button_if(!self.future.is_empty(), "Redo", || QuickMessage::Redo)
                            .separator()
                            .button("Add Bind", || QuickMessage::AddBind)
                            .button("Comp Tool", || QuickMessage::AddCompTool)
                            .pipe(Element::from)
                            .map(Message::Quick)
                            .map(OrRequest::Message)
                    },
                ),
            ))
            .into()
    }
}
