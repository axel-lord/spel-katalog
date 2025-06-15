use ::std::{
    ffi::{OsStr, OsString},
    ops::Mul,
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    time::Duration,
};

use ::clap::Parser;
use ::color_eyre::{Report, Section, eyre::eyre};
use ::derive_more::{From, IsVariant};
use ::iced::{
    Color, Element, Font,
    Length::{self, Fill},
    Subscription, Task,
    alignment::Horizontal::Left,
    keyboard::{self, Modifiers, on_key_press},
    widget::{
        self, button, container, horizontal_rule, horizontal_space, opaque, stack, text,
        text_input, toggler, value, vertical_space,
    },
};
use ::rustix::process::{Pid, RawPid};
use ::spel_katalog_common::{OrRequest, StatusSender, status, w};
use ::spel_katalog_info::{image_buffer::ImageBuffer, y};
use ::spel_katalog_settings::Variants;
use ::tap::Pipe;

mod view;

#[derive(Debug, Parser)]
#[command(author, version)]
pub struct Cli {
    #[command(flatten)]
    pub settings: ::spel_katalog_settings::Settings,

    /// Show settings at startup.
    #[arg(long)]
    pub show_settings: bool,

    /// Print a skeleton config.
    #[arg(long)]
    pub skeleton: bool,

    /// Config file to load.
    #[arg(long, short)]
    pub config: Option<PathBuf>,
}

#[derive(Debug)]
pub struct App {
    settings: ::spel_katalog_settings::State,
    games: ::spel_katalog_games::State,
    status: String,
    filter: String,
    view: view::State,
    info: ::spel_katalog_info::State,
    image_buffer: ImageBuffer,
    sender: StatusSender,
    process_list: Option<Vec<ProcessInfo>>,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    level: usize,
    pid: i64,
    name: Option<String>,
    cmdline: String,
}

impl ProcessInfo {
    pub fn view_list<'e>(list: &'e [ProcessInfo]) -> Element<'e, Message> {
        container(
            list.iter()
                .fold(w::col().push("Process Tree"), |col, info| {
                    col.push(info.view())
                })
                .align_x(Left)
                .pipe(w::scroll)
                .pipe(container)
                .style(container::bordered_box)
                .padding(3),
        )
        .center(Fill)
        .style(|_theme| container::background(Color::from_rgba8(0, 0, 0, 0.7)))
        .pipe(opaque)
        .into()
    }

    pub fn view<'e>(&'e self) -> Element<'e, Message> {
        let Self {
            level,
            pid,
            name,
            cmdline,
        } = self;
        let pid = *pid;
        let level = *level;
        w::row()
            .spacing(6)
            .push(horizontal_space().width(Length::Fixed(level.min(24).mul(12) as f32)))
            .push(
                button("X")
                    .padding(3)
                    .style(button::danger)
                    .on_press_with(move || Message::Kill(pid)),
            )
            .push(value(pid))
            .push_maybe(name.as_ref().map(text))
            .push(text(cmdline).font(Font::MONOSPACE))
            .into()
    }
}

#[derive(Debug, Clone, Copy, Default, IsVariant, PartialEq, Eq, Hash)]
pub enum Safety {
    None,
    #[default]
    Firejail,
}

impl From<bool> for Safety {
    fn from(value: bool) -> Self {
        if value { Self::Firejail } else { Self::None }
    }
}

#[derive(Debug, IsVariant, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuickMessage {
    ClosePane,
    ToggleSettings,
    OpenProcessInfo,
    CycleHidden,
    CycleFilter,
    ToggleNetwork,
    RefreshProcessInfo,
}

#[derive(Debug, IsVariant, From, Clone)]
pub enum Message {
    #[from]
    Status(String),
    Filter(String),
    #[from]
    Settings(::spel_katalog_settings::Message),
    #[from]
    View(view::Message),
    #[from]
    Games(OrRequest<::spel_katalog_games::Message, ::spel_katalog_games::Request>),
    #[from]
    Info(OrRequest<::spel_katalog_info::Message, ::spel_katalog_info::Request>),
    RunGame(i64, Safety),
    #[from]
    Quick(QuickMessage),
    ProcessInfo(Option<Vec<ProcessInfo>>),
    Kill(i64),
}

impl App {
    pub fn run() -> ::color_eyre::Result<()> {
        ::color_eyre::install()?;
        [
            "spel_katalog",
            "spel_katalog_common",
            "spel_katalog_settings",
            "spel_katalog_games",
        ]
        .into_iter()
        .fold(&mut ::env_logger::builder(), |builder, module| {
            builder.filter_module(module, ::log::LevelFilter::Debug)
        })
        .init();

        let rx;
        let app = {
            let Cli {
                settings,
                show_settings,
                skeleton,
                config,
            } = Cli::parse();

            let overrides = settings;
            let settings = if let Some(config) = &config {
                ::std::fs::read_to_string(config)
                    .map_err(|err| {
                        eyre!(err).suggestion(format!("does {config:?} exist, and is it readable"))
                    })?
                    .pipe_deref(::toml::from_str::<::spel_katalog_settings::Settings>)
                    .map_err(|err| eyre!(err).suggestion(format!("is {config:?} a toml file")))?
                    .apply(::spel_katalog_settings::Delta::create(overrides.clone()))
            } else {
                overrides.clone()
            };

            if skeleton {
                ::std::io::copy(
                    &mut ::std::io::Cursor::new(
                        ::toml::to_string_pretty(&settings.skeleton()).map_err(|err| eyre!(err))?,
                    ),
                    &mut ::std::io::stdout().lock(),
                )
                .map_err(|err| eyre!(err))?;

                ::std::process::exit(0)
            }

            let tx;
            (tx, rx) = ::tokio::sync::mpsc::channel(64);

            let filter = String::new();
            let status = String::new();
            let view = view::State::new(show_settings);
            let settings = ::spel_katalog_settings::State { settings, config };
            let games = ::spel_katalog_games::State::default();
            let image_buffer = ImageBuffer::empty();
            let info = ::spel_katalog_info::State::default();
            let sender = tx.into();
            let process_list = None;

            App {
                process_list,
                settings,
                status,
                filter,
                view,
                games,
                image_buffer,
                info,
                sender,
            }
        };

        ::iced::application("Lutris Games", Self::update, Self::view)
            .theme(|app| ::iced::Theme::from(*app.settings.settings.theme()))
            .centered()
            .subscription(Self::subscription)
            .executor::<::tokio::runtime::Runtime>()
            .run_with(|| {
                let load_db = app
                    .settings
                    .lutris_db()
                    .as_path()
                    .to_path_buf()
                    .pipe(spel_katalog_games::Message::LoadDb)
                    .pipe(OrRequest::Message)
                    .pipe(Message::Games)
                    .pipe(Task::done);
                let receive_status =
                    Task::stream(::tokio_stream::wrappers::ReceiverStream::new(rx))
                        .map(Message::Status);
                (app, Task::batch([receive_status, load_db]))
            })
            .map_err(Report::from)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        on_key_press(|key, modifiers| match key.as_ref() {
            keyboard::Key::Named(_named) => None,
            keyboard::Key::Character(chr) => match chr {
                "q" if modifiers.is_empty() => Some(QuickMessage::ClosePane),
                "h" if modifiers.is_empty() => Some(QuickMessage::CycleHidden),
                "f" if modifiers.is_empty() => Some(QuickMessage::CycleFilter),
                "s" if modifiers.is_empty() => Some(QuickMessage::ToggleSettings),
                "n" if modifiers.is_empty() => Some(QuickMessage::ToggleNetwork),
                "k" if modifiers == Modifiers::CTRL | Modifiers::SHIFT => {
                    Some(QuickMessage::OpenProcessInfo)
                }
                _ => None,
            },
            keyboard::Key::Unidentified => None,
        })
        .map(Message::Quick)
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Status(status) => {
                self.set_status(status);
                return Task::none();
            }
            Message::Filter(filter) => {
                self.filter = filter;
                self.games.sort(&self.settings, &self.filter);
            }
            Message::Settings(message) => {
                let re_sort = matches!(
                    &message,
                    ::spel_katalog_settings::Message::Delta(
                        ::spel_katalog_settings::Delta::FilterMode(..)
                            | ::spel_katalog_settings::Delta::Show(..)
                            | ::spel_katalog_settings::Delta::SortBy(..)
                            | ::spel_katalog_settings::Delta::SortDir(..)
                    )
                );

                let task = self.settings.update(message, &self.sender);

                if re_sort {
                    self.sort_games();
                }

                return task.map(From::from);
            }
            Message::View(message) => return self.view.update(message),
            Message::Games(message) => {
                let request = match message {
                    OrRequest::Message(message) => {
                        return self
                            .games
                            .update(message, &self.sender, &self.settings, &self.filter)
                            .map(Message::Games);
                    }
                    OrRequest::Request(request) => request,
                };
                match request {
                    ::spel_katalog_games::Request::SetId { id } => {
                        return self
                            .info
                            .update(
                                ::spel_katalog_info::Message::SetId { id },
                                &self.sender,
                                &self.settings,
                                &self.games,
                            )
                            .map(Message::Info);
                    }
                    ::spel_katalog_games::Request::Run { id, sandbox } => {
                        return self.run_game(
                            id,
                            if sandbox {
                                Safety::Firejail
                            } else {
                                Safety::None
                            },
                        );
                    }
                    ::spel_katalog_games::Request::FindImages { slugs } => {
                        return self
                            .image_buffer
                            .find_images(
                                slugs,
                                self.settings.coverart_dir().as_path().to_path_buf(),
                            )
                            .map(OrRequest::Message)
                            .map(Message::Games);
                    }
                }
            }
            Message::Info(message) => {
                let request = match message {
                    OrRequest::Message(message) => {
                        return self
                            .info
                            .update(message, &self.sender, &self.settings, &self.games)
                            .map(Message::Info);
                    }
                    OrRequest::Request(request) => request,
                };
                match request {
                    ::spel_katalog_info::Request::ShowInfo(show) => {
                        return self.view.update(view::Message::Info(show));
                    }
                    ::spel_katalog_info::Request::SetImage { slug, image } => {
                        return self
                            .games
                            .update(
                                ::spel_katalog_games::Message::SetImage { slug, image },
                                &self.sender,
                                &self.settings,
                                &self.filter,
                            )
                            .map(Message::Games);
                    }
                    ::spel_katalog_info::Request::RunGame { id, sandbox } => {
                        return self.run_game(id, Safety::from(sandbox));
                    }
                }
            }
            Message::RunGame(id, safety) => return self.run_game(id, safety),
            Message::Quick(quick) => match quick {
                QuickMessage::ClosePane => {
                    if self.process_list.is_some() {
                        self.process_list = None;
                        self.set_status("closed process list");
                    } else if self.view.info_shown() {
                        self.view.show_info(false);
                        self.set_status("closed info pane");
                    } else if self.view.settings_shown() {
                        self.view.show_settings(false);
                        self.set_status("closed settings pane");
                    }
                }
                QuickMessage::ToggleSettings => {
                    self.view.show_settings(!self.view.settings_shown());
                }
                QuickMessage::OpenProcessInfo => {
                    return Task::future(async {
                        let mut task_dir = ::tokio::fs::read_dir("/proc/self/task/").await?;
                        let mut children = Vec::new();
                        while let Some(entry) = task_dir.next_entry().await? {
                            let path = entry.path().join("children");
                            let task_children = match ::tokio::fs::read_to_string(&path).await {
                                Ok(task_children) => task_children,
                                Err(err) => {
                                    ::log::error!("reading path {path:?}\n{err}");
                                    continue;
                                }
                            };

                            children.extend(task_children.lines().flat_map(|line| {
                                let line = line.trim();
                                if line.is_empty() {
                                    None
                                } else {
                                    line.parse::<i64>().ok().map(|i| (0usize, i))
                                }
                            }));
                        }

                        let mut summary = Vec::<ProcessInfo>::new();
                        while let Some((level, child)) = children.pop() {
                            let proc = PathBuf::from(format!("/proc/{child}"));

                            let status = proc.join("status");
                            let name = match ::tokio::fs::read(&status).await {
                                Ok(bytes) => {
                                    let mut name = None;
                                    for line in
                                        bytes.split(|c| *c == b'\n').map(|line| line.trim_ascii())
                                    {
                                        if let Some(line) = line.strip_prefix(b"Name:") {
                                            name = line
                                                .trim_ascii()
                                                .pipe(OsStr::from_bytes)
                                                .display()
                                                .to_string()
                                                .pipe(Some);
                                            break;
                                        }
                                    }
                                    name
                                }
                                Err(err) => {
                                    ::log::error!("while reading {status:?}\n{err}");
                                    None
                                }
                            };

                            let cmdline = proc.join("cmdline");

                            let mut cmdline = match ::tokio::fs::read(&cmdline).await {
                                Ok(cmdline) => cmdline,
                                Err(err) => {
                                    ::log::error!("while reading {cmdline:?}\n{err}");
                                    continue;
                                }
                            };

                            let next_level = level.saturating_add(1);

                            while cmdline.last() == Some(&b'\0') {
                                cmdline.pop();
                            }

                            let cmdline = cmdline
                                .split(|c| *c == b'\0')
                                .map(|bytes| OsStr::from_bytes(bytes).display().to_string())
                                .pipe(::shell_words::join);

                            let tasks = proc.join("task");
                            let mut tasks = match ::tokio::fs::read_dir(&tasks).await {
                                Ok(tasks) => tasks,
                                Err(err) => {
                                    ::log::error!("reading directory {tasks:?}\n{err}");
                                    continue;
                                }
                            };

                            while let Some(entry) = tasks.next_entry().await? {
                                let path = entry.path().join("children");
                                let task_children = match ::tokio::fs::read_to_string(&path).await {
                                    Ok(task_children) => task_children,
                                    Err(err) => {
                                        ::log::error!("reading path {path:?}\n{err}");
                                        continue;
                                    }
                                };

                                children.extend(task_children.lines().flat_map(|line| {
                                    let line = line.trim();
                                    if line.is_empty() {
                                        None
                                    } else {
                                        line.parse::<i64>().ok().map(|i| (next_level, i))
                                    }
                                }));
                            }

                            summary.push(ProcessInfo {
                                level,
                                pid: child,
                                name,
                                cmdline,
                            });
                        }

                        Ok::<_, ::tokio::io::Error>(summary)
                    })
                    .then(|result| match result {
                        Ok(summary) => summary
                            .pipe(Some)
                            .pipe(Message::ProcessInfo)
                            .pipe(Task::done),
                        Err(err) => {
                            ::log::error!("whilst collecting info\n{err}");
                            Task::none()
                        }
                    });
                }
                QuickMessage::CycleHidden => {
                    let next = self.settings.show().cycle();
                    self.settings.apply_from(next);
                    self.set_status(format!("cycled hidden to {next}"));
                    self.sort_games();
                }
                QuickMessage::CycleFilter => {
                    let next = self.settings.filter_mode().cycle();
                    self.settings.apply_from(next);
                    self.set_status(format!("cycled filter mode to {next}"));
                    self.sort_games();
                }
                QuickMessage::ToggleNetwork => {
                    let next = self.settings.network().cycle();
                    self.settings.apply_from(next);
                    self.set_status("toggled network to {next}");
                    self.sort_games();
                }
                QuickMessage::RefreshProcessInfo => {
                    if self.process_list.is_some() {
                        return QuickMessage::OpenProcessInfo
                            .pipe(Message::Quick)
                            .pipe(Task::done);
                    }
                }
            },
            Message::ProcessInfo(process_infos) => self.process_list = process_infos,
            Message::Kill(pid) => {
                let Ok(pid) = RawPid::try_from(pid) else {
                    return Task::none();
                };
                let Some(pid) = Pid::from_raw(pid) else {
                    return Task::none();
                };

                return Task::future(async move {
                    match ::tokio::task::spawn_blocking(move || {
                        ::rustix::process::kill_process(pid, ::rustix::process::Signal::TERM)
                    })
                    .await
                    {
                        Ok(result) => match result {
                            Ok(_) => ::log::info!(
                                "sent TERM to process {pid}",
                                pid = pid.as_raw_nonzero().get()
                            ),
                            Err(err) => ::log::error!(
                                "could not kill process {pid}\n{err}",
                                pid = pid.as_raw_nonzero().get()
                            ),
                        },
                        Err(err) => ::log::error!("could not spawn blocking thread\n{err}"),
                    };

                    ::tokio::time::sleep(Duration::from_millis(750)).await;

                    Message::Quick(QuickMessage::RefreshProcessInfo)
                });
            }
        }
        Task::none()
    }

    pub fn sort_games(&mut self) {
        self.games.sort(&self.settings, &self.filter);
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        let status = status.into();
        ::log::info!("status: {status}");
        self.status = status;
    }

    fn run_game(&mut self, id: i64, safety: Safety) -> Task<Message> {
        let Some(game) = self.games.by_id(id) else {
            status!(&self.sender, "could not run game with id {id}");
            return Task::none();
        };

        let lutris = self.settings.lutris_exe().clone();
        let firejail = self.settings.firejail_exe().clone();
        let slug = game.slug.clone();
        let name = game.name.clone();
        let is_net_disabled = self.settings.network().is_disabled();
        let configpath = self
            .settings
            .yml_dir()
            .as_path()
            .join(&game.configpath)
            .with_extension("yml");

        let task = Task::future(async move {
            let rungame = format!("lutris:rungame/{slug}");

            let status = match safety {
                Safety::None => {
                    ::tokio::process::Command::new(lutris)
                        .arg(rungame)
                        .kill_on_drop(true)
                        .status()
                        .await
                }
                Safety::Firejail => {
                    let common = ::tokio::fs::read_to_string(&configpath)
                        .await
                        .map_err(|err| ::log::error!("could not read {configpath:?}\n{err}"))
                        .ok()
                        .and_then(|content| {
                            y::Config::parse(&content)
                                .map_err(|err| {
                                    ::log::error!("could not parse {configpath:?}\n{err}")
                                })
                                .ok()
                        })
                        .map(|config| config.game.common_parent())
                        .unwrap_or_else(|| ::spel_katalog_settings::HOME.as_path().into());
                    let mut arg = OsString::from("--whitelist=");
                    arg.push(common);
                    arg.push("/");

                    ::tokio::process::Command::new(firejail)
                        .arg(arg)
                        .args(is_net_disabled.then_some("--net=none"))
                        .arg(lutris)
                        .arg(rungame)
                        .kill_on_drop(true)
                        .status()
                        .await
                }
            };

            match status {
                Ok(status) => format!("{name} exited with {status}").into(),
                Err(err) => {
                    ::log::error!("could not run {slug}\n{err}");
                    format!("could not run {slug}").into()
                }
            }
        });

        task
    }

    pub fn view(&self) -> Element<Message> {
        w::col()
            .padding(5)
            .spacing(0)
            .push(
                text_input(
                    match self.settings.settings.filter_mode() {
                        ::spel_katalog_settings::FilterMode::Filter => "filter...",
                        ::spel_katalog_settings::FilterMode::Search => "search...",
                        ::spel_katalog_settings::FilterMode::Regex => "regex...",
                    },
                    &self.filter,
                )
                .width(Fill)
                .padding(3)
                .on_input(Message::Filter),
            )
            .push(vertical_space().height(5))
            .push(if let Some(process_info) = &self.process_list {
                stack([
                    self.view
                        .view(&self.settings, &self.games, &self.info, true)
                        .into(),
                    ProcessInfo::view_list(&process_info),
                ])
                .pipe(Element::from)
            } else {
                self.view
                    .view(&self.settings, &self.games, &self.info, false)
                    .pipe(Element::from)
            })
            .push(horizontal_rule(2))
            .push(
                w::row()
                    .push(text(&self.status).width(Fill))
                    .push(text("Displayed").style(widget::text::secondary))
                    .push(value(self.games.displayed_count()))
                    .push(text("All").style(widget::text::secondary))
                    .push(value(self.games.all_count()))
                    .push(text("Network").style(widget::text::secondary))
                    .push(
                        toggler(self.settings.network().is_enabled())
                            .spacing(0)
                            .on_toggle(|net| {
                                Message::Settings(::spel_katalog_settings::Message::Delta(
                                    spel_katalog_settings::Delta::Network(match net {
                                        true => spel_katalog_settings::Network::Enabled,
                                        false => spel_katalog_settings::Network::Disabled,
                                    }),
                                ))
                            }),
                    )
                    .push(text("Settings").style(widget::text::secondary))
                    .push(
                        toggler(self.view.settings_shown())
                            .spacing(0)
                            .on_toggle(|show_settings| {
                                view::Message::Settings(show_settings).into()
                            }),
                    ),
            )
            .pipe(Element::from)
    }
}
