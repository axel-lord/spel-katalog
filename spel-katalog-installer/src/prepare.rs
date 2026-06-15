//! Preparation screen.

use ::std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Stdio,
};

use ::iced_core::{Element, Length, Size, alignment::Vertical};
use ::iced_runtime::Task;
use ::iced_widget as widget;
use ::rfd::AsyncFileDialog;
use ::spel_katalog_common::{
    IntoOrRequest, OrRequest, display_bytes,
    in_place::{Convene, MapSelf},
};
use ::spel_katalog_formats::{Bind, NativeGame, NativeRunner, Timestamp};
use ::spel_katalog_settings::{InstallLocale, Settings, ThmubnailSource};
use ::spel_katalog_widget::rule;
use ::tap::{Conv, Pipe, TapOptional};

/// Choice of executable.
#[derive(Debug, Clone)]
pub enum ExeChoice {
    /// A single exe is chosen.
    /// If not representable by a string
    /// lossy conversion is performed and the
    /// original path is included.
    Value(String),
    /// A list of executables that are available.
    /// Every entry has the same format as [Self::Value]
    /// The first value is the index of the chosen candidate.
    List(usize, Vec<String>),
}

impl ExeChoice {
    /// get current choice.
    pub fn current(&self) -> Option<&str> {
        match self {
            ExeChoice::Value(exe) => Some(exe),
            ExeChoice::List(idx, items) => items.get(*idx).as_ref().map(|s| s.as_str()),
        }
    }

    /// Get file extension of selected choice.
    pub fn extension(&self) -> Option<&str> {
        Path::new(self.current()?)
            .extension()
            .and_then(OsStr::to_str)
    }

    /// Check if choice has given extension.
    pub fn has_ext(&self, ext: &str) -> bool {
        self.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    }
}

/// Preparation messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// A choice list item was chosen.
    ChooseChoice(String),
    /// A runner was selected.
    ChooseRunner(NativeRunner),
    /// Width of of dir row was updated.
    SetWidth(f32),
    /// Set the exe value.
    SetExe(String),
    /// Set the thumbnail value.
    SetThumb(PathBuf),
    /// Add available locales.
    AddLocales(Vec<String>),
    /// Set the current locale.
    SetLocale(String),
    /// An exe should be selected.
    OpenExe,
    /// A thumbnail should be selected.
    OpenThumb,
    /// Copy game directory to clibboard.
    CopyDirectory,
    /// Copy exe to clipboard.
    CopyExe,
    /// Edit title.
    EditTitle(String),
    /// Set hidden status.
    SetHidden(bool),
    /// Set the move game status.
    SetMoveGame(bool),
    /// Copy title to clipboard.
    CopyTitle,
    /// Paste title.
    PasteTitle,
    /// Ok was pressed.
    Ok,
}

/// Preparation stage of installer.
#[derive(Debug, Clone)]
pub struct Prepare {
    /// Title of game.
    title: String,
    /// Parent path.
    parent: String,
    /// Executable choice.
    choice: ExeChoice,
    /// Runner in use.
    runner: NativeRunner,
    /// Is the game hidden.
    hidden: bool,
    /// Width of column box.
    column_width: Option<f32>,
    /// Thumbnail to use.
    thumbnail: Option<PathBuf>,
    /// Available locales.
    locales: Vec<String>,
    /// Current locale.
    locale: String,
    /// Should the game be moved.
    move_game: bool,
}

impl Prepare {
    /// Construct a new instance.
    pub fn new(settings: &Settings, parent: String, choice: ExeChoice) -> (Self, Task<Message>) {
        (
            Self {
                title: parent
                    .trim_end_matches('/')
                    .rsplit_once('/')
                    .map_or_else(String::new, |(_, title)| title.to_owned()),
                runner: if choice.has_ext("exe") {
                    NativeRunner::Wine
                } else {
                    NativeRunner::Linux
                },
                hidden: false,
                column_width: None,
                thumbnail: None,
                locales: Vec::from([String::new()]),
                locale: settings.get::<InstallLocale>().as_str().to_owned(),
                move_game: true,
                parent,
                choice,
            },
            Task::<Option<_>>::future(async {
                const FULL: &str = "localectl list-locales";
                const CMD: &str = "localectl";
                const ARG: &str = "list-locales";
                let child = ::smol::process::Command::new(CMD)
                    .arg(ARG)
                    .kill_on_drop(true)
                    .stdin(Stdio::null())
                    .stderr(Stdio::inherit())
                    .stdout(Stdio::piped())
                    .spawn()
                    .map_err(|err| ::log::warn!("could not spawn locale finder\n{err}"))
                    .ok()?;

                let output = child
                    .output()
                    .await
                    .map_err(|err| ::log::warn!("{FULL} could not run\n{err}"))
                    .ok()?;

                if !output.status.success() {
                    ::log::warn!("{FULL} failed\n{}", output.status);
                }

                output
                    .stdout
                    .split(|&b| b == b'\n')
                    .filter_map(|s| {
                        str::from_utf8(s)
                            .map_err(|err| {
                                ::log::warn!(
                                    "could not parse locale {} as utf-8\n{err}",
                                    display_bytes(s)
                                )
                            })
                            .map(|s| s.trim())
                            .ok()
                            .filter(|&s| !matches!(s, "" | "C.UTF-8"))
                            .map(ToOwned::to_owned)
                    })
                    .collect::<Vec<_>>()
                    .pipe(Message::AddLocales)
                    .pipe(Some)
            })
            .and_then(Task::done),
        )
    }

    /// Get game config from current values.
    pub fn get_config(&self) -> Option<NativeGame> {
        let exe = self.choice.current()?;
        let parent = PathBuf::from(self.parent.clone());
        let exe = parent.join(exe);
        let config = NativeGame {
            bind: Vec::from([Bind::mirrored(parent)]),
            hidden: self.hidden,
            drives: [('g', PathBuf::from("../.."))].into_iter().collect(),
            prefix: if self.runner.is_wine() {
                self.parent
                    .as_str()
                    .pipe(Path::new)
                    .join(".umu_pfx")
                    .pipe(Some)
            } else {
                None
            },
            env: if !self.locale.is_empty() {
                [("LANG".to_owned(), self.locale.clone())]
                    .into_iter()
                    .collect()
            } else {
                Default::default()
            },
            ..NativeGame::new(self.title.clone(), Timestamp::now(), exe, self.runner)
        };
        Some(config)
    }

    /// Update state with message.
    pub fn update(&mut self, message: Message, settings: &Settings) -> Task<super::Message> {
        match message {
            Message::SetHidden(status) => {
                self.hidden = status;
                Task::none()
            }
            Message::SetMoveGame(status) => {
                self.move_game = status;
                Task::none()
            }
            Message::ChooseChoice(value) => {
                if let ExeChoice::List(idx, list) = &mut self.choice
                    && let Some(pos) = list.iter().position(|e| e == &value)
                {
                    *idx = pos;
                    self.runner = if self.choice.has_ext("exe") {
                        NativeRunner::Wine
                    } else {
                        NativeRunner::Linux
                    };
                }
                Task::none()
            }
            Message::ChooseRunner(runner) => {
                self.runner = runner;
                Task::none()
            }
            Message::SetWidth(w) => {
                self.column_width = Some(w);
                Task::none()
            }
            Message::CopyDirectory => ::iced_runtime::clipboard::write(self.parent.clone()),
            Message::CopyExe => {
                if let Some(choice) = self.choice.current() {
                    ::iced_runtime::clipboard::write(choice.to_owned())
                } else {
                    Task::none()
                }
            }
            Message::SetExe(exe) => {
                self.choice = ExeChoice::Value(exe);
                Task::none()
            }
            Message::AddLocales(locales) => {
                ::log::info!("addding locales\n{locales:#?}");
                self.locales.extend(locales);
                Task::none()
            }
            Message::SetLocale(locale) => {
                self.locale = locale;
                Task::none()
            }
            Message::EditTitle(title) => {
                self.title = title;
                Task::none()
            }
            Message::CopyTitle => ::iced_runtime::clipboard::write(self.title.clone()),
            Message::PasteTitle => ::iced_runtime::clipboard::read()
                .and_then(|title| Task::done(Message::EditTitle(title).conv::<super::Message>())),
            Message::OpenExe => {
                let location = self.parent.clone().conv::<PathBuf>();
                Task::<Option<_>>::future(async move {
                    let file = AsyncFileDialog::new()
                        .set_title("Select Executable")
                        .set_directory(&location)
                        .pick_file()
                        .await
                        .tap_none(|| ::log::warn!("no file chosen"))?;

                    file.path()
                        .strip_prefix(&location)
                        .map_err(|err| {
                            ::log::warn!(
                                "could not strip {location:?} from {:?}\n{err}",
                                file.path()
                            )
                        })
                        .ok()?
                        .as_os_str()
                        .to_str()
                        .tap_none(|| {
                            ::log::warn!("path {:?} contains non utf-8 segments", file.path())
                        })?
                        .to_owned()
                        .pipe(Message::SetExe)
                        .conv::<super::Message>()
                        .pipe(Some)
                })
                .and_then(Task::done)
            }
            Message::OpenThumb => {
                let location = settings.get::<ThmubnailSource>().to_path_buf();
                Task::<Option<_>>::future(async move {
                    AsyncFileDialog::new()
                        .set_title("Select Thumbnail")
                        .set_directory(location)
                        .add_filter(
                            "image",
                            &[
                                "png", "jpg", "jpeg", "avif", "webp", "bmp", "tga", "tiff", "gif",
                                "ico", "pnm", "ff", "exr",
                            ],
                        )
                        .pick_file()
                        .await
                        .tap_none(|| ::log::warn!("not thumbnail chosen"))
                        .map(|file| file.path().to_path_buf())
                        .map(Message::SetThumb)
                        .map(super::Message::Prepare)
                })
                .and_then(Task::done)
            }
            Message::SetThumb(thumb) => {
                self.thumbnail = Some(thumb);
                Task::none()
            }
            Message::Ok => self
                .get_config()
                .map(Box::new)
                .map(super::Message::SetEditor)
                .map_or_else(Task::none, Task::done),
        }
    }

    /// View fields.
    pub fn view_fields(
        &self,
    ) -> ::iced_widget::Container<'_, OrRequest<super::Message, super::Request>> {
        widget::container(
            widget::Column::new()
                .pipe_some(self.column_width, |col, width| col.width(width))
                .convene()
                .spacing(3)
                .push(widget::text("Title"))
                .push(::iced_aw::widget::ContextMenu::new(
                    widget::text_input("title...", &self.title)
                        .on_input(|text| {
                            Message::EditTitle(text)
                                .conv::<super::Message>()
                                .into_message()
                        })
                        .width(
                            self.column_width
                                .map_or(Length::Fixed(0.0), |_| Length::Fill),
                        ),
                    || {
                        ::spel_katalog_widget::ListMenu::new()
                            .push(widget::text("title"))
                            .separator()
                            .button("Copy", || {
                                Message::CopyTitle.conv::<super::Message>().into_message()
                            })
                            .button("Paste", || {
                                Message::PasteTitle.conv::<super::Message>().into_message()
                            })
                            .into()
                    },
                ))
                .push(widget::text("Game Directory"))
                .push(
                    widget::Row::new()
                        .spacing(3)
                        .push(widget::button("Open...").padding(3).on_press_with(|| {
                            super::Message::SelectDir(Some(PathBuf::from(self.parent.as_str())))
                                .into_message()
                        }))
                        .push(
                            widget::button("Copy")
                                .padding(3)
                                .style(widget::button::success)
                                .on_press(
                                    Message::CopyDirectory
                                        .conv::<super::Message>()
                                        .into_message(),
                                ),
                        )
                        .push(
                            widget::container(
                                widget::text(&self.parent)
                                    .pipe_if(self.column_width.is_some(), |t| t.width(Length::Fill))
                                    .convene(),
                            )
                            .padding(3)
                            .style(widget::container::rounded_box),
                        ),
                )
                .push(widget::text("Executable"))
                .push(match &self.choice {
                    ExeChoice::Value(value) => widget::Row::new()
                        .spacing(3)
                        .push(
                            widget::button("Open...")
                                .padding(3)
                                .on_press(Message::OpenExe.conv::<super::Message>().into_message()),
                        )
                        .push(
                            widget::button("Copy")
                                .padding(3)
                                .style(widget::button::success)
                                .on_press(Message::CopyExe.conv::<super::Message>().into_message()),
                        )
                        .push(
                            widget::container(
                                widget::text(value)
                                    .pipe_if(self.column_width.is_some(), |t| t.width(Length::Fill))
                                    .convene(),
                            )
                            .style(widget::container::rounded_box)
                            .padding(3),
                        ),
                    ExeChoice::List(idx, items) => widget::Row::new()
                        .spacing(3)
                        .push(
                            widget::button("Open...")
                                .padding(3)
                                .on_press(Message::OpenExe.conv::<super::Message>().into_message()),
                        )
                        .push(
                            widget::button("Copy")
                                .padding(3)
                                .style(widget::button::success)
                                .on_press(Message::CopyExe.conv::<super::Message>().into_message()),
                        )
                        .push(
                            widget::pick_list(items.as_slice(), items.get(*idx), |i: String| {
                                Message::ChooseChoice(i)
                                    .conv::<super::Message>()
                                    .into_message()
                            })
                            .pipe_if(self.column_width.is_some(), |l| l.width(Length::Fill))
                            .convene()
                            .padding(3),
                        ),
                })
                .push(
                    widget::Row::new()
                        .spacing(3)
                        .align_y(Vertical::Center)
                        .pipe_if(self.column_width.is_some(), |r| r.push(rule::horizontal()))
                        .convene()
                        .push(
                            widget::button("Thumbnail")
                                .padding(3)
                                .on_press(
                                    super::Message::Prepare(Message::OpenThumb).into_message(),
                                )
                                .pipe_if(self.thumbnail.is_none(), |b| {
                                    b.style(widget::button::success)
                                })
                                .or_else(|b| b.style(widget::button::secondary)),
                        )
                        .push(rule::sized_horizontal(12))
                        .push(widget::text("Hidden"))
                        .push(widget::checkbox(self.hidden).on_toggle(|status| {
                            Message::SetHidden(status)
                                .conv::<super::Message>()
                                .into_message()
                        }))
                        .push(rule::sized_horizontal(12))
                        .push(widget::text("Move"))
                        .push(widget::checkbox(self.move_game).on_toggle(|status| {
                            Message::SetMoveGame(status)
                                .conv::<super::Message>()
                                .into_message()
                        }))
                        .pipe_if(self.column_width.is_some(), |r| r.push(rule::horizontal()))
                        .convene(),
                )
                .push(
                    widget::Row::new()
                        .spacing(3)
                        .align_y(Vertical::Center)
                        .pipe_if(self.column_width.is_some(), |r| r.push(rule::horizontal()))
                        .convene()
                        .push(widget::text("Runner"))
                        .push(
                            widget::pick_list(
                                NativeRunner::variants(),
                                Some(self.runner),
                                |runner| {
                                    Message::ChooseRunner(runner)
                                        .conv::<super::Message>()
                                        .into_message()
                                },
                            )
                            .padding(3),
                        )
                        .push(rule::sized_horizontal(12))
                        .push(widget::text("Locale"))
                        .push(
                            widget::pick_list(
                                self.locales.as_slice(),
                                Some(&self.locale),
                                |locale| {
                                    Message::SetLocale(locale)
                                        .conv::<super::Message>()
                                        .into_message()
                                },
                            )
                            .padding(3),
                        )
                        .pipe_if(self.column_width.is_some(), |r| r.push(rule::horizontal()))
                        .convene(),
                )
                .pipe_some(self.thumbnail.as_deref(), |col, thumb| {
                    col.push(
                        widget::container(widget::image(thumb).width(300))
                            .pipe_if(self.column_width.is_some(), |c| c.center_x(Length::Fill))
                            .convene(),
                    )
                })
                .convene()
                .pipe_if(self.column_width.is_none(), widget::sensor)
                .map(|sensor| {
                    let read_width = |size: Size| {
                        Message::SetWidth(size.width)
                            .pipe(super::Message::Prepare)
                            .into_message()
                    };
                    sensor.on_show(read_width).on_resize(read_width)
                })
                .map(::spel_katalog_widget::xy_scrollable)
                .or_else(::spel_katalog_widget::xy_scrollable),
        )
        .style(widget::container::bordered_box)
        .padding(6)
    }

    /// Display buttons.
    pub fn buttons(
        &self,
    ) -> ::iced_widget::Container<'_, OrRequest<super::Message, super::Request>> {
        widget::Row::new()
            .spacing(3)
            .push(
                widget::button("Cancel")
                    .padding(3)
                    .style(widget::button::danger)
                    .on_press(super::Request::Close.into_request()),
            )
            .push(
                widget::button("Ok")
                    .padding(3)
                    .style(widget::button::success)
                    .on_press(Message::Ok.conv::<super::Message>().into_message()),
            )
            .pipe(widget::container)
            .style(widget::container::bordered_box)
            .padding(4)
    }

    /// View state.
    pub fn view(
        &self,
    ) -> Element<
        '_,
        OrRequest<super::Message, super::Request>,
        ::iced_core::Theme,
        ::iced_widget::Renderer,
    > {
        widget::container(
            widget::Stack::new()
                .height(Length::Fill)
                .width(Length::Fill)
                .push(self.view_fields().pipe(widget::center_x))
                .push(self.buttons().pipe(widget::bottom_right)),
        )
        .padding(6)
        .into()
    }
}
