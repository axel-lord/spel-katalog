use ::std::{ffi::OsStr, path::PathBuf};

use ::derive_more::From;
use ::iced::{
    Alignment::{self, Center},
    Element, Font,
    Length::Fill,
    Task,
    widget::{self, button, horizontal_rule, horizontal_space, vertical_rule},
};
use ::image::ImageError;
use ::spel_katalog_common::{OrStatus, status};
use ::tap::Pipe;

use crate::{Safety, games::Games, image_buffer::ImageBuffer, settings::Settings, t, w, y};

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

#[derive(Debug, Clone, From)]
pub enum Message {
    SetId(i64),
    SetContent(i64, String, PathBuf),
    #[from]
    UpdateContent(widget::text_editor::Action),
    SaveContent,
    AddThumb(i64),
}

impl State {
    pub fn update(
        &mut self,
        message: Message,
        settings: &Settings,
        games: &Games,
    ) -> Task<OrStatus<crate::Message>> {
        match message {
            Message::SetId(id) => {
                self.id = id;

                let fill_content;

                if let Some(game) = games.by_id(id) {
                    let path = settings
                        .yml_dir()
                        .as_path()
                        .join(&game.configpath)
                        .with_extension("yml");

                    fill_content = Task::future(::tokio::fs::read_to_string(path.clone())).then(
                        move |result| match result {
                            Ok(value) => Message::SetContent(id, value, path.clone())
                                .pipe(crate::Message::from)
                                .pipe(OrStatus::new)
                                .pipe(Task::done),
                            Err(err) => {
                                ::log::error!("failed to read yml {path:?}\n{err}");
                                Task::done(status!("could not read {path:?}"))
                            }
                        },
                    );
                } else {
                    fill_content = Task::none();
                }

                let show_info = crate::view::Message::Info(true)
                    .pipe(crate::Message::from)
                    .pipe(OrStatus::new)
                    .pipe(Task::done);

                Task::batch([fill_content, show_info])
            }
            Message::SetContent(id, content, path) => {
                if id == self.id {
                    self.content = widget::text_editor::Content::with_text(&content);
                    self.config_path = Some(path.clone());

                    let yml = match ::serde_yml::from_str::<y::Config>(&content) {
                        Ok(yml) => yml,
                        Err(err) => {
                            ::log::error!("could not parse yml {path:?}\n{err}");
                            return Task::done(format!("could not parse {path:?}").into());
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
                    Task::future(::tokio::fs::write(path.clone(), self.content.text())).then(
                        move |result| match result {
                            Ok(_) => Task::done(status!("wrote game config {path:?}")),
                            Err(err) => {
                                ::log::error!("could not write config {path:?}\n{err}");
                                Task::done(status!("could not write config {path:?}"))
                            }
                        },
                    )
                } else {
                    Task::none()
                }
            }
            Message::AddThumb(id) => match games.by_id(id) {
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

                        Ok(crate::Message::Games(crate::games::Message::SetImage {
                            slug,
                            image,
                        }))
                    };

                    Task::future(task).then(|result| match result {
                        Ok(msg) => Task::done(OrStatus::new(msg)),
                        Err(err) => {
                            ::log::error!("{err}");
                            Task::done(status!("could not add thumbnail"))
                        }
                    })
                }
                None => Task::none(),
            },
        }
    }

    pub fn view<'a>(
        &'a self,
        _settings: &'a Settings,
        games: &'a Games,
    ) -> Element<'a, crate::Message> {
        let Some(game) = games.by_id(self.id) else {
            return w::col()
                .align_x(Center)
                .push("No Game Selected")
                .push(horizontal_rule(2))
                .into();
        };

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
                                        widget::value(&game.id)
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
                            .on_press(crate::Message::RunGame(self.id, Safety::Firejail)),
                    )
                    .push(
                        button("Run")
                            .padding(3)
                            .style(widget::button::danger)
                            .on_press(crate::Message::RunGame(self.id, Safety::None)),
                    )
                    .push(
                        button("Save").padding(3).on_press_maybe(
                            self.config_path
                                .is_some()
                                .then(|| Message::SaveContent.into()),
                        ),
                    )
                    .push(
                        button("+Thumb").padding(3).on_press_maybe(
                            game.image
                                .is_none()
                                .then(|| Message::AddThumb(game.id).into()),
                        ),
                    )
                    .push(horizontal_space())
                    .push(
                        widget::button("Close")
                            .padding(3)
                            .style(widget::button::secondary)
                            .on_press_with(|| {
                                crate::Message::View(crate::view::Message::Info(false))
                            }),
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
                        .on_action(|action| action.pipe(Message::from).pipe(crate::Message::from)),
                ),
            ))
            .into()
    }
}
