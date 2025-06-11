//! Game info view.
#![allow(missing_docs)]

use ::std::{convert::identity, ffi::OsStr, path::PathBuf};

use ::derive_more::{From, IsVariant};
use ::iced::{
    Alignment::{self, Center},
    Element, Font,
    Length::Fill,
    Task,
    widget::{self, button, horizontal_rule, horizontal_space, image::Handle, vertical_rule},
};
use ::image::ImageError;
use ::spel_katalog_common::{OrRequest, StatusSender, async_status, status, t, w};
use ::spel_katalog_games::Games;
use ::spel_katalog_settings::Settings;
use ::tap::Pipe;

use crate::image_buffer::ImageBuffer;

pub mod image_buffer;
pub mod y;

#[derive(Debug)]
pub struct State {
    id: i64,
    content: widget::text_editor::Content,
    config_path: Option<PathBuf>,
    common_parent: PathBuf,
}

impl Default for State {
    fn default() -> Self {
        Self {
            id: i64::MAX,
            content: widget::text_editor::Content::default(),
            config_path: None,
            common_parent: PathBuf::from("/"),
        }
    }
}

#[derive(Debug, Clone, From, IsVariant)]
pub enum Message {
    SetId {
        id: i64,
    },
    SetContent {
        id: i64,
        content: String,
        path: PathBuf,
    },
    #[from]
    UpdateContent(widget::text_editor::Action),
    SaveContent,
    AddThumb {
        /// Game id to add thumbnail for
        id: i64,
    },
}

#[derive(Debug, Clone, IsVariant)]
pub enum Request {
    ShowInfo(bool),
    SetImage { slug: String, image: Handle },
    RunGame { id: i64, sandbox: bool },
}

impl State {
    pub fn update(
        &mut self,
        message: Message,
        tx: &StatusSender,
        settings: &Settings,
        games: &Games,
    ) -> Task<OrRequest<Message, Request>> {
        match message {
            Message::SetId { id } => {
                self.id = id;

                let fill_content;

                if let Some(game) = games.by_id(id) {
                    let path = settings
                        .yml_dir()
                        .as_path()
                        .join(&game.configpath)
                        .with_extension("yml");

                    let tx = tx.clone();
                    fill_content = Task::future(async move {
                        match ::tokio::fs::read_to_string(&path).await {
                            Ok(value) => Message::SetContent {
                                id,
                                content: value,
                                path: path.clone(),
                            }
                            .pipe(OrRequest::Message)
                            .pipe(Task::done),
                            Err(err) => {
                                ::log::error!("failed to read yml {path:?}\n{err}");
                                async_status!(tx, "could not read {path:?}").await;
                                Task::none()
                            }
                        }
                    })
                    .then(identity);
                } else {
                    fill_content = Task::none();
                }

                let show_info = Request::ShowInfo(true)
                    .pipe(OrRequest::Request)
                    .pipe(Task::done);

                Task::batch([fill_content, show_info])
            }
            Message::SetContent { id, content, path } => {
                if id == self.id {
                    self.content = widget::text_editor::Content::with_text(&content);
                    self.config_path = Some(path.clone());

                    let yml = match ::serde_yml::from_str::<y::Config>(&content) {
                        Ok(yml) => yml,
                        Err(err) => {
                            ::log::error!("could not parse yml {path:?}\n{err}");
                            status!(tx, "could not parse {path:?}");
                            return Task::none();
                        }
                    };

                    self.common_parent = yml.game.common_parent();
                }

                Task::none()
            }
            Message::UpdateContent(action) => {
                self.content.perform(action);
                Task::none()
            }
            Message::SaveContent => {
                if let Some(path) = &self.config_path {
                    let path = path.to_path_buf();
                    let tx = tx.clone();
                    let text = self.content.text();
                    Task::future(async move {
                        match ::tokio::fs::write(&path, text).await {
                            Ok(_) => {
                                async_status!(tx, "wrote game config {path:?}").await;
                                Task::none()
                            }
                            Err(err) => {
                                ::log::error!("could not write config {path:?}\n{err}");
                                async_status!(tx, "could not write config {path:?}").await;
                                Task::none()
                            }
                        }
                    })
                    .then(identity)
                } else {
                    Task::none()
                }
            }
            Message::AddThumb { id } => match games.by_id(id) {
                Some(game) => {
                    #[derive(Debug, thiserror::Error)]
                    enum AddThumbError {
                        #[error("no thumbnail chosen")]
                        NoneChosen,
                        #[error("could not copy {from:?} to {to:?}\n{source}")]
                        Copy {
                            #[source]
                            source: ::std::io::Error,
                            from: PathBuf,
                            to: PathBuf,
                        },
                        #[error("could not read {path:?}\n{source}")]
                        Read {
                            #[source]
                            source: ::std::io::Error,
                            path: PathBuf,
                        },
                        #[error("could not process {path:?}\n{source}")]
                        Process {
                            #[source]
                            source: ImageError,
                            path: PathBuf,
                        },
                    }

                    let dest = settings.coverart_dir().as_path().join(&game.slug);
                    let slug = game.slug.clone();

                    let task = async move {
                        let dialog = ::rfd::AsyncFileDialog::new()
                            .set_title("Add Thumbnail")
                            .pick_file()
                            .await
                            .ok_or_else(|| AddThumbError::NoneChosen)?;

                        let dest = dest
                            .with_extension(dialog.path().extension().unwrap_or(OsStr::new("")));

                        match ::tokio::fs::copy(dialog.path(), &dest).await {
                            Ok(_) => (),
                            Err(source) => {
                                return Err(AddThumbError::Copy {
                                    source,
                                    from: dialog.path().to_path_buf(),
                                    to: dest,
                                });
                            }
                        }

                        let content = match ::tokio::fs::read(&dest).await {
                            Ok(content) => content,
                            Err(source) => return Err(AddThumbError::Read { source, path: dest }),
                        };

                        let image = match ImageBuffer::process_bytes(&content) {
                            Ok(handle) => handle,
                            Err(source) => {
                                return Err(AddThumbError::Process { source, path: dest });
                            }
                        };

                        Ok(OrRequest::Request(Request::SetImage { slug, image }))
                    };

                    let tx = tx.clone();
                    Task::future(async move {
                        match task.await {
                            Ok(msg) => Task::done(msg),

                            Err(err) => {
                                ::log::error!("{err}");
                                async_status!(tx, "could not add thumbnail").await;
                                Task::none()
                            }
                        }
                    })
                    .then(identity)
                }
                None => Task::none(),
            },
        }
    }

    pub fn view<'a>(
        &'a self,
        _settings: &'a Settings,
        games: &'a Games,
    ) -> Element<'a, OrRequest<Message, Request>> {
        let Some(game) = games.by_id(self.id) else {
            return w::col()
                .align_x(Center)
                .push("No Game Selected")
                .push(horizontal_rule(2))
                .into();
        };
        let id = game.id;

        w::col()
            .push(
                w::row()
                    .align_y(Alignment::Start)
                    .height(150)
                    .push_maybe(game.image.as_ref().map(|image| widget::image(image)))
                    .push_maybe(game.image.is_some().then(|| widget::vertical_rule(2)))
                    .push(
                        w::col()
                            .push(
                                w::row().push(widget::text(&game.name).width(Fill).align_x(Center)),
                            )
                            .push(horizontal_rule(2))
                            .push(
                                w::row()
                                    .push(widget::text("Runner").font(Font::MONOSPACE))
                                    .push(vertical_rule(2))
                                    .push(
                                        widget::value(&game.runner)
                                            .font(Font::MONOSPACE)
                                            .align_x(Alignment::Start)
                                            .width(Fill),
                                    )
                                    .push(vertical_rule(2)),
                            )
                            .push(horizontal_rule(2))
                            .push(
                                w::row()
                                    .push(widget::text("Slug  ").font(Font::MONOSPACE))
                                    .push(vertical_rule(2))
                                    .push(
                                        widget::value(&game.slug)
                                            .font(Font::MONOSPACE)
                                            .align_x(Alignment::Start)
                                            .width(Fill),
                                    )
                                    .push(vertical_rule(2)),
                            )
                            .push(horizontal_rule(2))
                            .push(
                                w::row()
                                    .push(widget::text("Id    ").font(Font::MONOSPACE))
                                    .push(vertical_rule(2))
                                    .push(
                                        widget::value(id)
                                            .font(Font::MONOSPACE)
                                            .align_x(Alignment::Start)
                                            .width(Fill),
                                    )
                                    .push(vertical_rule(2)),
                            ),
                    ),
            )
            .push(horizontal_rule(2))
            .push(
                w::row()
                    .push(
                        button("Sandbox")
                            .padding(3)
                            .style(widget::button::success)
                            .on_press(OrRequest::Request(Request::RunGame { id, sandbox: true })),
                    )
                    .push(
                        button("Run")
                            .padding(3)
                            .style(widget::button::danger)
                            .on_press(OrRequest::Request(Request::RunGame { id, sandbox: false })),
                    )
                    .push(
                        button("Save").padding(3).on_press_maybe(
                            self.config_path
                                .is_some()
                                .then(|| OrRequest::Message(Message::SaveContent)),
                        ),
                    )
                    .push(
                        button("+Thumb").padding(3).on_press_maybe(
                            game.image
                                .is_none()
                                .then(|| OrRequest::Message(Message::AddThumb { id })),
                        ),
                    )
                    .push(horizontal_space())
                    .push(
                        widget::button("Close")
                            .padding(3)
                            .style(widget::button::secondary)
                            .on_press_with(|| OrRequest::Request(Request::ShowInfo(false))),
                    ),
            )
            .push(horizontal_rule(2))
            .push("Directory")
            .push(
                widget::container(widget::value(self.common_parent.display()))
                    .width(Fill)
                    .padding(3)
                    .style(t::box_border),
            )
            .push(w::scroll(
                w::col().push(
                    widget::text_editor(&self.content)
                        .on_action(|action| action.pipe(Message::from).pipe(OrRequest::Message)),
                ),
            ))
            .into()
    }
}
