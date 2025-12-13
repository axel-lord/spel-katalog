//! Game info view.

use ::core::convert::identity;
use ::std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::Arc,
};

use ::derive_more::{From, IsVariant};
use ::iced_core::{
    Alignment::{self, Center},
    Font,
    Length::Fill,
};
use ::iced_highlighter::Highlighter;
use ::iced_runtime::Task;
use ::iced_widget::{
    self as widget, button,
    text_editor::{Action, Edit},
};
use ::image::ImageError;
use ::open::that;
use ::spel_katalog_common::{OrRequest, PushMaybe, StatusSender, async_status, status, styling, w};
use ::spel_katalog_formats::AdditionalConfig;
use ::spel_katalog_settings::{ConfigDir, CoverartDir, Settings, YmlDir};
use ::tap::Pipe;
use ::yaml_rust2::Yaml;

pub mod formats;

mod attrs;

/// Element alias.
type Element<'a, M> = ::iced_core::Element<'a, M, ::iced_core::Theme, ::iced_renderer::Renderer>;

/// State of info display.
#[derive(Debug)]
pub struct State {
    /// Id of current game.
    id: i64,
    /// Content of config editor.
    content: widget::text_editor::Content,
    /// Path to game config.
    config_path: Option<PathBuf>,
    /// Common parent of wine prefix and executable.
    common_parent: PathBuf,
    /// Content of additional roots editor.
    additional_roots_content: widget::text_editor::Content,
    /// Additional config of game.
    additional: AdditionalConfig,
    /// Attribute editor.
    attrs: attrs::State,
}

impl Default for State {
    fn default() -> Self {
        Self {
            id: i64::MAX,
            content: widget::text_editor::Content::default(),
            additional_roots_content: Default::default(),
            config_path: None,
            common_parent: PathBuf::from("/"),
            additional: AdditionalConfig::default(),
            attrs: attrs::State::default(),
        }
    }
}

/// Message used by info display.
#[derive(Debug, Clone, From, IsVariant)]
pub enum Message {
    /// Set content of config editor.
    SetContent {
        /// Id of game to verify match.
        id: i64,
        /// Content to set editor to.
        content: String,
        /// Path to game config.
        path: PathBuf,
        /// Additional config.
        additional: AdditionalConfig,
    },
    /// Update config editor content.
    #[from]
    UpdateContent(widget::text_editor::Action),
    /// Update additional roots editor content.
    UpdateAdditionalRoots(widget::text_editor::Action),
    /// Update attribute editor.
    UpdateAttrs(attrs::Message),
    /// Save config content to file.
    SaveContent,
    /// Save additional config to file.
    SaveAdditional,
    /// Add a thumbail.
    AddThumb {
        /// Game id to add thumbnail for
        id: i64,
    },
    /// Remove a thumbail.
    RemoveThumb {
        /// Game id to removremovee thumbnail for
        id: i64,
    },
    /// Set executable in game config.
    SetExe {
        /// Path to new executable.
        path: PathBuf,
    },
    /// Open executable selection dialog.
    OpenExe,
    /// Open directory of game.
    OpenDir,
    /// Clear viewed content.
    Clear,
}

/// Request application for action.
#[derive(Debug, Clone, IsVariant)]
pub enum Request {
    /// Set image of game wiht given slug.
    SetImage {
        /// Slug of game to set image for.
        slug: String,
        /// Image to set.
        image: ::spel_katalog_formats::Image,
    },
    /// Remove image for game with given slug.
    RemoveImage {
        /// Slug of game to remove image for.
        slug: String,
    },
    /// Run the game with given id.
    RunGame {
        /// Id of game to run.
        id: i64,
        /// Should game be sandboxed.
        sandbox: bool,
    },
    /// Run lutris in sandbox of game.
    RunLutrisInSandbox {
        /// Id of game to run lutris in sandbox of.
        id: i64,
    },
}

impl State {
    /// Get id of currently viewed game.
    pub const fn id(&self) -> Option<i64> {
        if self.id == i64::MAX {
            None
        } else {
            Some(self.id)
        }
    }

    /// Clear viewed contents.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Set the id of the currently viewed game, and generate a task that finds
    /// info for it.
    pub fn set_game(
        &mut self,
        tx: &StatusSender,
        settings: &Settings,
        game: &::spel_katalog_formats::Game,
    ) -> Task<Message> {
        let id = game.id;

        // We leave the rest of the content to avoid being empty.
        self.id = id;

        let path = settings
            .get::<YmlDir>()
            .as_path()
            .join(&game.configpath)
            .with_extension("yml");

        let additional_path = settings
            .get::<ConfigDir>()
            .as_path()
            .join("games")
            .join(format!("{id}.toml"));

        async fn read_additional(path: &Path) -> Option<AdditionalConfig> {
            // Usually a bad idea however since we deal with it not existing correctly
            // anyways this is just some redundancy that prevents log spam.
            if !path.exists() {
                return None;
            };
            ::smol::fs::read_to_string(path)
                .await
                .map_err(|err| ::log::error!("could not read {path:?} to string\n{err}"))
                .ok()?
                .pipe_deref(::toml::from_str)
                .map_err(|err| ::log::error!("could not deserialize {path:?}\n{err}"))
                .ok()
        }

        let tx = tx.clone();
        Task::future(async move {
            match ::smol::fs::read_to_string(&path).await {
                Ok(value) => {
                    let additional = read_additional(&additional_path).await.unwrap_or_default();
                    Task::done(Message::SetContent {
                        id,
                        content: value,
                        path: path.clone(),
                        additional,
                    })
                }
                Err(err) => {
                    ::log::error!("failed to read yml {path:?}\n{err}");
                    async_status!(tx, "could not read {path:?}").await;
                    Task::done(Message::Clear)
                }
            }
        })
        .then(identity)
    }

    /// Udate state of info display.
    pub fn update<'a>(
        &'a mut self,
        message: Message,
        tx: &'a StatusSender,
        settings: &'a Settings,
        game_by_id: &dyn Fn(i64) -> Option<&'a ::spel_katalog_formats::Game>,
    ) -> Task<OrRequest<Message, Request>> {
        match message {
            Message::Clear => {
                self.clear();
                Task::none()
            }
            Message::SetContent {
                id,
                content,
                path,
                additional,
            } => {
                // Verify id matches.
                if id != self.id {
                    return Task::none();
                }

                self.set_content(content.clone());

                self.config_path = Some(path.clone());
                self.additional_roots_content =
                    widget::text_editor::Content::with_text(&additional.sandbox_root.join("\n"));
                self.attrs = attrs::State::default();
                self.attrs.attrs = additional
                    .attrs
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();
                self.additional = additional;

                // Move to task?
                let yml = match formats::Config::parse(&content) {
                    Ok(yml) => yml,
                    Err(err) => {
                        ::log::error!("could not parse yml {path:?}\n{err}");
                        status!(tx, "could not parse {path:?}");
                        return Task::none();
                    }
                };

                self.common_parent = yml.game.common_parent();

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
                        match ::smol::fs::write(&path, text).await {
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
            Message::AddThumb { id } => match game_by_id(id) {
                Some(game) => {
                    #[derive(Debug, thiserror::Error)]
                    enum AddThumbError {
                        /// No thumbnail was chosen.
                        #[error("no thumbnail chosen")]
                        NoneChosen,
                        /// A file could not be copied.
                        #[error("could not copy {from:?} to {to:?}\n{source}")]
                        Copy {
                            /// Error that occured.
                            #[source]
                            source: ::std::io::Error,
                            /// Path to file that was to be copied.
                            from: PathBuf,
                            /// Path it was to be copied to.
                            to: PathBuf,
                        },
                        /// A file could not be read.
                        #[error("could not read {path:?}\n{source}")]
                        Read {
                            /// Error that occurred.
                            #[source]
                            source: ::std::io::Error,
                            /// Path to file.
                            path: PathBuf,
                        },
                        /// Thumbnail could not be processed.
                        #[error("could not process {path:?}\n{source}")]
                        Process {
                            /// Error that occurred.
                            #[source]
                            source: ImageError,
                            /// Path to image.
                            path: PathBuf,
                        },
                    }

                    let dest = settings.get::<CoverartDir>().as_path().join(&game.slug);
                    let slug = game.slug.clone();

                    let task = async move {
                        let dialog = ::rfd::AsyncFileDialog::new()
                            .set_title("Add Thumbnail")
                            .add_filter("png, jpg", &["png", "jpg", "jpeg"])
                            .pick_file()
                            .await
                            .ok_or_else(|| AddThumbError::NoneChosen)?;

                        let dest = dest
                            .with_extension(dialog.path().extension().unwrap_or(OsStr::new("")));

                        match ::smol::fs::copy(dialog.path(), &dest).await {
                            Ok(_) => (),
                            Err(source) => {
                                return Err(AddThumbError::Copy {
                                    source,
                                    from: dialog.path().to_path_buf(),
                                    to: dest,
                                });
                            }
                        }

                        let content = match ::smol::fs::read(&dest).await {
                            Ok(content) => content,
                            Err(source) => return Err(AddThumbError::Read { source, path: dest }),
                        };

                        let image = match ::image::load_from_memory(&content) {
                            Ok(handle) => handle,
                            Err(source) => {
                                return Err(AddThumbError::Process { source, path: dest });
                            }
                        };

                        let image = ::spel_katalog_gather::thumbnail(
                            image,
                            ::spel_katalog_gather::CoverGathererOptions::default().dimensions,
                        );

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
            Message::RemoveThumb { id } => {
                let Some(game) = game_by_id(id) else {
                    return Task::none();
                };

                let dest = settings.get::<CoverartDir>().as_path().join(&game.slug);
                let slug = game.slug.clone();
                let tx = tx.clone();

                let task = async move {
                    const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];

                    for &ext in EXTENSIONS {
                        let path = dest.with_extension(ext);
                        if let Err(err) = ::smol::fs::remove_file(&path).await {
                            ::log::warn!("could not remove {path:?}\n{err}");
                        } else {
                            async_status!(tx, "removed {path:?}").await;
                        }
                    }

                    OrRequest::Request(Request::RemoveImage { slug })
                };
                Task::future(task)
            }
            Message::UpdateAdditionalRoots(action) => {
                self.additional_roots_content.perform(action);

                self.additional.sandbox_root = self
                    .additional_roots_content
                    .lines()
                    .filter(|line| !line.text.trim().is_empty())
                    .map(|s| s.text.trim().to_owned())
                    .collect();

                Task::none()
            }
            Message::UpdateAttrs(msg) => {
                let task = self.attrs.update(msg);

                self.additional.attrs = self
                    .attrs
                    .attrs
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect();

                task.map(Message::UpdateAttrs).map(OrRequest::Message)
            }
            Message::SaveAdditional => {
                async fn write_additional(path: &Path, additional: AdditionalConfig) -> Option<()> {
                    let content = ::toml::to_string(&additional)
                        .map_err(|err| {
                            ::log::error!("could not serialize additional to {path:?}\n{err}")
                        })
                        .ok()?;
                    ::smol::fs::write(path, content.as_bytes())
                        .await
                        .map_err(|err| ::log::error!("could not write to {path:?}\n{err}"))
                        .ok()
                }

                let id = self.id;
                let additional = self.additional.clone();
                let extra_config_dir = settings.get::<ConfigDir>().as_path().join("games");

                let tx = tx.clone();
                Task::future(async move {
                    if let Err(err) = ::smol::fs::create_dir_all(&extra_config_dir).await {
                        async_status!(tx, "could not create {extra_config_dir:?}").await;
                        ::log::error!(
                            "could not create extra config dir {extra_config_dir:?}\n{err}"
                        );
                        return;
                    };
                    let additional_path = extra_config_dir.join(format!("{id}.toml"));
                    match write_additional(&additional_path, additional).await {
                        Some(_) => async_status!(tx, "saved additional for game {id}").await,
                        None => async_status!(tx, "could not save addotional for game {id}").await,
                    }
                })
                .then(|_| Task::none())
            }
            Message::SetExe { path } => {
                self.set_exe(path, tx);
                Task::none()
            }
            Message::OpenExe => self.open_exe(tx).unwrap_or_else(Task::none),
            Message::OpenDir => {
                let Some(game) = game_by_id(self.id) else {
                    status!(tx, "could not get game by id {}", self.id);
                    return Task::none();
                };

                let config_path = settings
                    .get::<YmlDir>()
                    .as_path()
                    .join(&game.configpath)
                    .with_extension("yml");

                let tx = tx.clone();

                Task::future(async move {
                    let content = match ::smol::fs::read_to_string(&config_path).await {
                        Ok(content) => content,
                        Err(err) => {
                            async_status!(&tx, "could not read {config_path:?}").await;
                            ::log::error!("while reading {config_path:?}\n{err}");
                            return;
                        }
                    };

                    let config = match formats::Config::parse(&content) {
                        Ok(config) => config,
                        Err(err) => {
                            async_status!(&tx, "could not parse {config_path:?}").await;
                            ::log::error!("while parsing {config_path:?} as yaml\n{err}");
                            return;
                        }
                    };

                    let parent = match config.game.exe.canonicalize() {
                        Ok(mut exe_path) => {
                            if exe_path.pop() {
                                exe_path
                            } else {
                                async_status!(&tx, "could not get parent of {exe_path:?}").await;
                                ::log::error!("could not get parent of {exe_path:?}");
                                return;
                            }
                        }
                        Err(err) => {
                            async_status!(&tx, "could not canonicalize {:?}", config.game.exe)
                                .await;
                            ::log::error!("while canonicalizing {:?}\n{err}", config.game.exe);
                            return;
                        }
                    };

                    if let Err(err) = that(&parent) {
                        async_status!(&tx, "failed to open {parent:?}").await;
                        ::log::error!("failed to open {parent:?}\n{err}");
                    }

                    async_status!(&tx, "opened {parent:?}").await;
                })
                .then(|_| Task::none())
            }
        }
    }

    /// Set config editor content.
    fn set_content(&mut self, content: String) {
        // Probably most correct solution.
        // self.content = widget::text_editor::Content::with_text(&content);

        // Unless performed as actions, formatting is ignored for some reason
        [
            Action::SelectAll,
            Action::Edit(Edit::Delete),
            Action::Edit(Edit::Paste(Arc::new(content))),
        ]
        .into_iter()
        .for_each(|action| self.content.perform(action));
    }

    /// Open dialog to replace exe in config.
    fn open_exe(&mut self, tx: &StatusSender) -> Option<Task<OrRequest<Message, Request>>> {
        let tx = tx.clone();
        let exe = formats::Config::parse(&self.content.text())
            .map_err(|err| {
                ::log::error!("could not load yaml\n{err}");
                status!(&tx, "could not load yaml");
            })
            .ok()?
            .game
            .exe
            .to_path_buf();

        let Some(dir) = formats::Config::parse(&self.content.text())
            .map_err(|err| {
                ::log::error!("could not load yaml\n{err}");
                status!(&tx, "could not load yaml");
            })
            .ok()?
            .game
            .exe
            .parent()
            .map(Path::to_path_buf)
        else {
            status!(tx, "could not get parent of {exe:?}");
            ::log::error!("could not get parent of {exe:?}");
            return None;
        };

        let task = Task::future(async {
            let dialog = ::rfd::AsyncFileDialog::new()
                .set_title("Choose Exe")
                .set_directory(dir)
                .pick_file()
                .await?;

            Some(OrRequest::Message(Message::SetExe {
                path: dialog.path().to_path_buf(),
            }))
        })
        .then(|msg| match msg {
            Some(value) => Task::done(value),
            None => Task::none(),
        });

        Some(task)
    }

    /// Set exe in config.
    fn set_exe(&mut self, path: PathBuf, tx: &StatusSender) -> Option<()> {
        let path = path.to_str().map(String::from)?;

        let yml_content = self.content.text();
        let mut yml = ::yaml_rust2::YamlLoader::load_from_str(&yml_content)
            .map_err(|err| {
                ::log::error!("could not load yaml\n{err}");
                status!(tx, "could not load yaml");
            })
            .ok()?;

        let exe = yml
            .first_mut()?
            .as_mut_hash()?
            .get_mut(&formats::GAME)?
            .as_mut_hash()?
            .get_mut(&formats::EXE)?;

        *exe = Yaml::String(path);

        let mut text = String::new();
        let mut emitter = ::yaml_rust2::YamlEmitter::new(&mut text);
        for yml in yml {
            emitter
                .dump(&yml)
                .map_err(|err| {
                    ::log::error!("could not emit yaml\n{err}");
                    status!(tx, "could not emit yaml");
                })
                .ok()?;
        }

        let pfx = "---\n";
        if text.starts_with(pfx) {
            _ = text.drain(..pfx.len());
        }

        self.set_content(text);

        Some(())
    }

    /// View game info.
    pub fn titlebar<'a, M: 'a>(
        &'a self,
        game: &'a ::spel_katalog_formats::Game,
        thumb: Option<&'a widget::image::Handle>,
        id: i64,
        buttons: Element<'a, M>,
    ) -> Element<'a, M> {
        w::row()
            .align_y(Alignment::Start)
            .height(150)
            .push_maybe(thumb.map(|thumb| widget::image(thumb).width(150).height(150)))
            .push_maybe(thumb.is_some().then(|| widget::rule::vertical(2)))
            .push(
                w::col()
                    .push(
                        w::row()
                            .push(widget::text(&game.name).width(Fill).align_x(Center))
                            .push(buttons),
                    )
                    .push(widget::rule::horizontal(2))
                    .push(
                        w::row()
                            .push(widget::text("Runner").font(Font::MONOSPACE))
                            .push(widget::rule::vertical(2))
                            .push(
                                widget::value(&game.runner)
                                    .font(Font::MONOSPACE)
                                    .align_x(Alignment::Start)
                                    .width(Fill),
                            ),
                    )
                    .push(widget::rule::horizontal(2))
                    .push(
                        w::row()
                            .push(widget::text("Slug  ").font(Font::MONOSPACE))
                            .push(widget::rule::vertical(2))
                            .push(
                                widget::value(&game.slug)
                                    .font(Font::MONOSPACE)
                                    .align_x(Alignment::Start)
                                    .width(Fill),
                            ),
                    )
                    .push(widget::rule::horizontal(2))
                    .push(
                        w::row()
                            .push(widget::text("Id    ").font(Font::MONOSPACE))
                            .push(widget::rule::vertical(2))
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

    /// View info.
    pub fn view<'a>(&'a self, has_thumb: bool) -> Element<'a, OrRequest<Message, Request>> {
        let id = self.id;
        widget::Column::new()
            .spacing(3)
            .push(
                [
                    button("Sandbox")
                        .style(widget::button::success)
                        .on_press(OrRequest::Request(Request::RunGame { id, sandbox: true })),
                    button("Run")
                        .style(widget::button::danger)
                        .on_press(OrRequest::Request(Request::RunGame { id, sandbox: false })),
                    button("Lutris")
                        .on_press(OrRequest::Request(Request::RunLutrisInSandbox { id })),
                    button("+Thumb").padding(3).on_press_maybe(
                        (!has_thumb).then(|| OrRequest::Message(Message::AddThumb { id })),
                    ),
                    button("-Thumb")
                        .style(widget::button::danger)
                        .on_press_maybe(
                            has_thumb.then(|| OrRequest::Message(Message::RemoveThumb { id })),
                        ),
                    button("Open").on_press(OrRequest::Message(Message::OpenDir)),
                ]
                .into_iter()
                .fold(w::row(), |row, btn| row.push(btn.padding(3))),
            )
            .push(widget::rule::horizontal(2))
            .push(w::scroll(
                widget::Column::new()
                    .spacing(3)
                    .push("Root Directories")
                    .push(if self.additional.sandbox_root.is_empty() {
                        widget::value(self.common_parent.display())
                            .pipe(widget::container)
                            .width(Fill)
                            .padding(3)
                            .style(|t| styling::box_border(t).background(t.palette().background))
                            .pipe(Element::from)
                    } else {
                        let mut iter = self.additional.sandbox_root.iter().map(widget::text);
                        w::col()
                            .push_maybe(iter.next())
                            .extend(
                                iter.flat_map(|row| {
                                    [widget::rule::horizontal(2).into(), row.into()]
                                }),
                            )
                            .pipe(widget::container)
                            .width(Fill)
                            .padding(3)
                            .style(|t| styling::box_border(t).background(t.palette().background))
                            .into()
                    })
                    .push(
                        w::row()
                            .push("Additional")
                            .push(widget::space::horizontal())
                            .push(
                                button("Save")
                                    .padding(3)
                                    .on_press(OrRequest::Message(Message::SaveAdditional)),
                            ),
                    )
                    .push(widget::rule::horizontal(2))
                    .push("Sandbox Roots")
                    .push(
                        widget::text_editor(&self.additional_roots_content)
                            .on_action(|action| {
                                action
                                    .pipe(Message::UpdateAdditionalRoots)
                                    .pipe(OrRequest::Message)
                            })
                            .padding(3),
                    )
                    .push("Attributes")
                    .push(
                        self.attrs
                            .view()
                            .map(Message::UpdateAttrs)
                            .map(OrRequest::Message),
                    )
                    .push(widget::rule::horizontal(2))
                    .push(
                        w::row()
                            .push(widget::container("Game Yml").padding(3))
                            .push(widget::space::horizontal())
                            .push(
                                button("Exe")
                                    .padding(3)
                                    .on_press_with(|| OrRequest::Message(Message::OpenExe)),
                            )
                            .push(
                                button("Save").padding(3).on_press_maybe(
                                    self.config_path
                                        .is_some()
                                        .then(|| OrRequest::Message(Message::SaveContent)),
                                ),
                            ),
                    )
                    .push(widget::themer(
                        Some(::iced_core::Theme::SolarizedDark),
                        widget::text_editor(&self.content)
                            .highlight_with::<Highlighter>(
                                ::iced_highlighter::Settings {
                                    theme: ::iced_highlighter::Theme::SolarizedDark,
                                    token: "yml".to_owned(),
                                },
                                |h, _| h.to_format(),
                            )
                            .on_action(|action| action.pipe(Message::from).pipe(OrRequest::Message))
                            .padding(6),
                    ))
                    .push(widget::space::horizontal().width(0)),
            ))
            .into()
    }
}
