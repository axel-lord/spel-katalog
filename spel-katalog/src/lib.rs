use ::std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

use ::clap::Parser;
use ::color_eyre::{Report, Section, eyre::eyre};
use ::derive_more::{From, IsVariant};
use ::iced::{
    Element,
    Length::Fill,
    Subscription, Task,
    keyboard::{self, Modifiers, key::Named, on_key_press},
    widget::{self, horizontal_rule, stack, text, text_input, toggler, value, vertical_space},
};
use ::rustix::process::{Pid, RawPid};
use ::spel_katalog_common::{OrRequest, StatusSender, status, w};
use ::spel_katalog_games::SelDir;
use ::spel_katalog_info::{
    formats::{self, Additional},
    image_buffer::ImageBuffer,
};
use ::spel_katalog_settings::{
    CoverartDir, ExtraConfigDir, FilterMode, FirejailExe, LutrisDb, LutrisExe, Network, Show,
    Theme, Variants, YmlDir,
};
use ::tap::Pipe;

mod view;

fn default_config() -> &'static Path {
    static LAZY: OnceLock<PathBuf> = OnceLock::new();
    LAZY.get_or_init(|| {
        let mut cfg = PathBuf::from(::spel_katalog_settings::HOME.as_str());
        cfg.push(".config");
        cfg.push("spel-katalog");
        cfg.push("config.toml");
        cfg
    })
}

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
    #[arg(long, short, default_value=default_config().as_os_str())]
    pub config: PathBuf,
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
    process_list: Option<Vec<process_info::ProcessInfo>>,
}

mod process_info;

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
    RunSelected,
    Next,
    Prev,
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
    ProcessInfo(Option<Vec<process_info::ProcessInfo>>),
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

            fn read_settings(
                path: &Path,
            ) -> ::color_eyre::Result<::spel_katalog_settings::Settings> {
                ::std::fs::read_to_string(path)
                    .map_err(|err| {
                        eyre!(err).suggestion(format!("does {path:?} exist, and is it readable"))
                    })?
                    .pipe_deref(::toml::from_str::<::spel_katalog_settings::Settings>)
                    .map_err(|err| eyre!(err).suggestion(format!("is {path:?} a toml file")))
            }

            let overrides = settings;
            let settings = read_settings(&config)
                .map_err(|err| ::log::error!("could not read config file {config:?}\n{err}"))
                .unwrap_or_default()
                .apply(::spel_katalog_settings::Delta::create(overrides));

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
            .theme(|app| ::iced::Theme::from(*app.settings.settings.get::<Theme>()))
            .centered()
            .subscription(Self::subscription)
            .executor::<::tokio::runtime::Runtime>()
            .run_with(|| {
                let load_db = app
                    .settings
                    .get::<LutrisDb>()
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
        fn sel(sel_dir: SelDir) -> Option<Message> {
            sel_dir
                .pipe(::spel_katalog_games::Message::Select)
                .pipe(OrRequest::Message)
                .pipe(Message::Games)
                .pipe(Some)
        }
        on_key_press(|key, modifiers| match key.as_ref() {
            keyboard::Key::Named(named) => match named {
                Named::Tab if modifiers.is_empty() => Some(QuickMessage::Next).map(Message::Quick),
                Named::Tab if modifiers == Modifiers::SHIFT => {
                    Some(QuickMessage::Prev).map(Message::Quick)
                }
                Named::ArrowRight if modifiers.is_empty() => sel(SelDir::Right),
                Named::ArrowLeft if modifiers.is_empty() => sel(SelDir::Left),
                Named::ArrowUp if modifiers.is_empty() => sel(SelDir::Up),
                Named::ArrowDown if modifiers.is_empty() => sel(SelDir::Down),
                Named::Enter | Named::Space if modifiers.is_empty() => {
                    Some(Message::Quick(QuickMessage::RunSelected))
                }
                _ => None,
            },
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
            }
            .map(Message::Quick),
            keyboard::Key::Unidentified => None,
        })
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
                                self.settings.get::<CoverartDir>().as_path().to_path_buf(),
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
                    return Task::future(process_info::ProcessInfo::open()).then(|result| {
                        match result {
                            Ok(summary) => summary
                                .pipe(Some)
                                .pipe(Message::ProcessInfo)
                                .pipe(Task::done),
                            Err(err) => {
                                ::log::error!("whilst collecting info\n{err}");
                                Task::none()
                            }
                        }
                    });
                }
                QuickMessage::CycleHidden => {
                    let next = self.settings.get::<Show>().cycle();
                    self.settings.apply_from(next);
                    self.set_status(format!("cycled hidden to {next}"));
                    self.sort_games();
                }
                QuickMessage::CycleFilter => {
                    let next = self.settings.get::<FilterMode>().cycle();
                    self.settings.apply_from(next);
                    self.set_status(format!("cycled filter mode to {next}"));
                    self.sort_games();
                }
                QuickMessage::ToggleNetwork => {
                    let next = self.settings.get::<Network>().cycle();
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
                QuickMessage::Next => return widget::focus_next(),
                QuickMessage::Prev => return widget::focus_previous(),
                QuickMessage::RunSelected => {
                    if let Some(id) = self.games.selected() {
                        return self.run_game(id, Safety::Firejail);
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

        let lutris = self.settings.get::<LutrisExe>().clone();
        let firejail = self.settings.get::<FirejailExe>().clone();
        let slug = game.slug.clone();
        let name = game.name.clone();
        let is_net_disabled = self.settings.get::<Network>().is_disabled();
        let configpath = self
            .settings
            .get::<YmlDir>()
            .as_path()
            .join(&game.configpath)
            .with_extension("yml");
        let extra_config_path = self
            .settings
            .get::<ExtraConfigDir>()
            .as_path()
            .join(format!("{id}.toml"));

        let task = Task::future(async move {
            let rungame = format!("lutris:rungameid/{id}");

            fn wl(p: impl AsRef<OsStr>) -> OsString {
                let mut s = OsString::new();
                s.push("--whitelist=");
                s.push(p);
                s
            }

            let status = match safety {
                Safety::None => {
                    ::log::info!("executing {lutris:?} with arguments\n{:#?}", &[&rungame]);
                    ::tokio::process::Command::new(lutris)
                        .arg(rungame)
                        .kill_on_drop(true)
                        .status()
                        .await
                }
                Safety::Firejail => {
                    let mut args;
                    if extra_config_path.exists() {
                        let Some(content) = ::tokio::fs::read_to_string(&extra_config_path)
                            .await
                            .map_err(|err| {
                                ::log::error!("could not read {extra_config_path:?}\n{err}")
                            })
                            .ok()
                        else {
                            return format!("could not read {extra_config_path:?}").into();
                        };
                        let Some(additional) = ::toml::from_str::<Additional>(&content)
                            .map_err(|err| {
                                ::log::error!("could not parse {extra_config_path:?}\n{err}")
                            })
                            .ok()
                        else {
                            return format!("could not parse {extra_config_path:?}").into();
                        };
                        args = additional
                            .sandbox_root
                            .into_iter()
                            .map(wl)
                            .collect::<Vec<_>>();
                    } else {
                        args = ::tokio::fs::read_to_string(&configpath)
                            .await
                            .map_err(|err| ::log::error!("could not read {configpath:?}\n{err}"))
                            .ok()
                            .and_then(|content| {
                                formats::Config::parse(&content)
                                    .map_err(|err| {
                                        ::log::error!("could not parse {configpath:?}\n{err}")
                                    })
                                    .ok()
                            })
                            .map(|config| config.game.common_parent())
                            .unwrap_or_else(|| ::spel_katalog_settings::HOME.as_path().into())
                            .pipe(::std::iter::once)
                            .map(wl)
                            .collect::<Vec<_>>();
                    }

                    if is_net_disabled {
                        args.push("--net=none".into());
                    }

                    args.push(lutris.as_os_str().into());
                    args.push(rungame.into());

                    ::log::info!("executing {firejail:?} with arguments\n{args:#?}");

                    ::tokio::process::Command::new(firejail)
                        .args(args)
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
                    match self.settings.settings.get::<FilterMode>() {
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
                    process_info::ProcessInfo::view_list(&process_info),
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
                        toggler(self.settings.get::<Network>().is_enabled())
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
