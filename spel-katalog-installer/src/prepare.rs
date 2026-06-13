//! Preparation screen.

use ::std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use ::iced_core::{Element, Length, alignment::Horizontal};
use ::iced_runtime::Task;
use ::iced_widget as widget;
use ::rfd::AsyncFileDialog;
use ::spel_katalog_formats::NativeRunner;
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
    SetTitleWidth(::iced_core::Size),
    /// Set the exe value.
    SetExe(String),
    /// An exe should be selected.
    OpenExe,
    /// Copy game directory to clibboard.
    CopyDirectory,
    /// Copy exe to clipboard.
    CopyExe,
    /// Edit title.
    EditTitle(String),
    /// Copy title to clipboard.
    CopyTitle,
    /// Paste title.
    PasteTitle,
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
    /// Width of title row.
    title_width: f32,
}

impl Prepare {
    /// Construct a new instance.
    pub fn new(parent: String, choice: ExeChoice) -> Self {
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
            title_width: 100.0,
            parent,
            choice,
        }
    }

    /// Update state with message.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
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
            Message::SetTitleWidth(size) => {
                self.title_width = size.width.max(100.0);
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
            Message::EditTitle(title) => {
                self.title = title;
                Task::none()
            }
            Message::CopyTitle => ::iced_runtime::clipboard::write(self.title.clone()),
            Message::PasteTitle => ::iced_runtime::clipboard::read()
                .and_then(|title| Task::done(Message::EditTitle(title))),
            Message::OpenExe => {
                let location = self.parent.clone().conv::<PathBuf>();
                Task::<Option<_>>::future(async move {
                    let file = AsyncFileDialog::new()
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
                        .pipe(Some)
                })
                .and_then(Task::done)
            }
        }
    }

    /// View state.
    pub fn view(&self) -> Element<'_, super::Message, ::iced_core::Theme, ::iced_widget::Renderer> {
        widget::Column::new()
            .padding(6)
            .align_x(Horizontal::Center)
            .push(
                widget::container(
                    widget::Column::new()
                        .spacing(3)
                        .push(widget::text("Title"))
                        .push(::iced_aw::widget::ContextMenu::new(
                            widget::text_input("title...", &self.title)
                                .width(self.title_width)
                                .on_input(|text| Message::EditTitle(text).conv::<super::Message>()),
                            || {
                                ::spel_katalog_widget::ListMenu::new()
                                    .push(widget::text("title"))
                                    .separator()
                                    .button("Copy", || Message::CopyTitle.conv::<super::Message>())
                                    .button("Paste", || {
                                        Message::PasteTitle.conv::<super::Message>()
                                    })
                                    .into()
                            },
                        ))
                        .push(widget::text("Game Directory"))
                        .push(
                            ::iced_widget::sensor(
                                widget::Row::new()
                                    .spacing(3)
                                    .push(widget::button("Open...").padding(3).on_press_with(
                                        || {
                                            super::Message::SelectDir(Some(PathBuf::from(
                                                self.parent.as_str(),
                                            )))
                                        },
                                    ))
                                    .push(
                                        widget::button("Copy")
                                            .padding(3)
                                            .style(widget::button::success)
                                            .on_press(
                                                Message::CopyDirectory.conv::<super::Message>(),
                                            ),
                                    )
                                    .push(
                                        widget::container(widget::text(&self.parent))
                                            .padding(3)
                                            .style(widget::container::rounded_box),
                                    ),
                            )
                            .on_show(|size| Message::SetTitleWidth(size).conv::<super::Message>())
                            .on_resize(|size| {
                                Message::SetTitleWidth(size).conv::<super::Message>()
                            }),
                        )
                        .push(widget::text("Executable"))
                        .push(match &self.choice {
                            ExeChoice::Value(value) => widget::Row::new()
                                .spacing(3)
                                .push(
                                    widget::button("Open...")
                                        .padding(3)
                                        .on_press(Message::OpenExe.conv::<super::Message>()),
                                )
                                .push(
                                    widget::button("Copy")
                                        .padding(3)
                                        .style(widget::button::success)
                                        .on_press(Message::CopyExe.conv::<super::Message>()),
                                )
                                .push(
                                    widget::container(widget::text(value))
                                        .style(widget::container::rounded_box)
                                        .padding(3),
                                ),
                            ExeChoice::List(idx, items) => widget::Row::new()
                                .spacing(3)
                                .push(
                                    widget::button("Open...")
                                        .padding(3)
                                        .on_press(Message::OpenExe.conv::<super::Message>()),
                                )
                                .push(
                                    widget::button("Copy")
                                        .padding(3)
                                        .style(widget::button::success)
                                        .on_press(Message::CopyExe.conv::<super::Message>()),
                                )
                                .push(
                                    widget::pick_list(
                                        items.as_slice(),
                                        items.get(*idx),
                                        |i: String| Message::ChooseChoice(i).into(),
                                    )
                                    .padding(3),
                                ),
                        })
                        .push(widget::text("Runner"))
                        .push(
                            widget::pick_list(
                                NativeRunner::variants(),
                                Some(self.runner),
                                |runner| Message::ChooseRunner(runner).conv::<super::Message>(),
                            )
                            .padding(3),
                        ),
                )
                .style(widget::container::bordered_box)
                .padding(6),
            )
            .push(widget::space().height(Length::Fill))
            .push(
                widget::Row::new()
                    .spacing(3)
                    .push(widget::space().width(Length::Fill))
                    .push(
                        widget::button("Cancel")
                            .padding(3)
                            .style(widget::button::danger)
                            .on_press(super::Message::Reset),
                    )
                    .push(
                        widget::button("Ok")
                            .padding(3)
                            .style(widget::button::success),
                    ),
            )
            .into()
    }
}
