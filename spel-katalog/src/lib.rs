use ::std::{
    convert::identity,
    ffi::{OsStr, OsString},
    io::{BufWriter, Write},
    mem,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::Duration,
};

use ::clap::{CommandFactory, Parser, Subcommand};
use ::color_eyre::{Report, Section, eyre::eyre};
use ::derive_more::{From, IsVariant};
use ::iced::{
    Element,
    Length::Fill,
    Subscription, Task,
    futures::{TryStreamExt, stream::FuturesOrdered},
    keyboard::{self, Modifiers, key::Named, on_key_press},
    widget::{self, horizontal_rule, stack, text, text_input, toggler, value, vertical_space},
};
use ::rustix::process::{Pid, RawPid};
use ::spel_katalog_batch::BatchInfo;
use ::spel_katalog_common::{OrRequest, StatusSender, status, w};
use ::spel_katalog_games::SelDir;
use ::spel_katalog_info::{
    formats::{self, Additional},
    image_buffer::ImageBuffer,
};
use ::spel_katalog_script::script_file::ScriptFile;
use ::spel_katalog_settings::{
    CoverartDir, ExtraConfigDir, FilterMode, FirejailExe, LutrisDb, LutrisExe, Network,
    ScriptConfigDir, Show, Theme, Variants, YmlDir,
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

    /// Config file to load.
    #[arg(long, short, default_value=default_config().as_os_str())]
    pub config: PathBuf,

    /// Perform an action other than opening gui.
    #[command(subcommand)]
    pub action: Option<Subcmd>,
}

fn get_shell() -> ::clap_complete::Shell {
    ::clap_complete::Shell::from_env().unwrap_or_else(|| ::clap_complete::Shell::Bash)
}

/// Use cases other than launching gui.
#[derive(Debug, Subcommand)]
pub enum Subcmd {
    /// Output a skeleton config.
    Skeleton {
        /// Where to write skeleton to.
        #[arg(long, short, default_value = "-")]
        output: PathBuf,
    },
    /// Output completions.
    Completions {
        /// Shell to use.
        #[arg(short, long, value_enum, default_value_t = get_shell())]
        shell: ::clap_complete::Shell,
        /// Name of the binary completions should be generated for.
        #[arg(short, long, default_value = "spel-katalog")]
        name: String,
        /// Where to write completions to.
        #[arg(short, long, default_value = "-")]
        output: PathBuf,
    },
}

#[derive(Debug)]
pub struct App {
    settings: ::spel_katalog_settings::State,
    games: ::spel_katalog_games::State,
    status: String,
    filter: String,
    view: view::State,
    info: ::spel_katalog_info::State,
    batch: ::spel_katalog_batch::State,
    show_batch: bool,
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
    CloseAll,
    ToggleSettings,
    OpenProcessInfo,
    CycleHidden,
    CycleFilter,
    ToggleNetwork,
    RefreshProcessInfo,
    RunSelected,
    Next,
    Prev,
    ToggleBatch,
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
    #[from]
    Batch(OrRequest<::spel_katalog_batch::Message, ::spel_katalog_batch::Request>),
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
                config,
                action,
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

            if let Some(action) = action {
                match action {
                    Subcmd::Skeleton { output } => {
                        let mut stdout;
                        let mut file;
                        let writer: &mut dyn Write;
                        if output.as_os_str().to_str() == Some("-") {
                            stdout = ::std::io::stdout().lock();
                            writer = &mut stdout;
                        } else {
                            file = ::std::fs::File::create(&output)
                                .map(BufWriter::new)
                                .map_err(|err| {
                                    eyre!("could not create/open {output:?}").error(err)
                                })?;
                            writer = &mut file;
                        }
                        ::std::io::copy(
                            &mut ::std::io::Cursor::new(
                                ::toml::to_string_pretty(&settings.skeleton())
                                    .map_err(|err| eyre!(err))?,
                            ),
                            writer,
                        )
                        .map_err(|err| eyre!(err))?;
                        writer
                            .flush()
                            .map_err(|err| eyre!("could not close/flush {output:?}").error(err))?;
                        ::std::process::exit(0)
                    }
                    Subcmd::Completions {
                        shell,
                        name,
                        output,
                    } => {
                        if output.as_os_str().to_str() == Some("-") {
                            ::clap_complete::generate(
                                shell,
                                &mut Cli::command(),
                                name,
                                &mut ::std::io::stdout().lock(),
                            );
                        } else {
                            let mut writer = ::std::fs::File::create(&output)
                                .map(BufWriter::new)
                                .map_err(|err| {
                                eyre!("could not create/open {output:?}").error(err)
                            })?;
                            ::clap_complete::generate(
                                shell,
                                &mut Cli::command(),
                                name,
                                &mut writer,
                            );
                        }
                        ::std::process::exit(0)
                    }
                }
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
            let show_batch = false;
            let batch = Default::default();

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
                batch,
                show_batch,
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
        let on_key = on_key_press(|key, modifiers| match key.as_ref() {
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
                "q" if modifiers == Modifiers::CTRL => Some(QuickMessage::CloseAll),
                "h" if modifiers.is_empty() => Some(QuickMessage::CycleHidden),
                "f" if modifiers.is_empty() => Some(QuickMessage::CycleFilter),
                "s" if modifiers.is_empty() => Some(QuickMessage::ToggleSettings),
                "n" if modifiers.is_empty() => Some(QuickMessage::ToggleNetwork),
                "k" if modifiers == Modifiers::CTRL | Modifiers::SHIFT => {
                    Some(QuickMessage::OpenProcessInfo)
                }
                "b" if modifiers == Modifiers::CTRL | Modifiers::SHIFT => {
                    Some(QuickMessage::ToggleBatch)
                }
                _ => None,
            }
            .map(Message::Quick),
            keyboard::Key::Unidentified => None,
        });

        let refresh = if self.process_list.is_some() {
            Some(
                ::iced::time::every(Duration::from_millis(500))
                    .map(|_| Message::Quick(QuickMessage::RefreshProcessInfo)),
            )
        } else {
            None
        };

        [Some(on_key), refresh]
            .into_iter()
            .flatten()
            .pipe(Subscription::batch)
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
                            false,
                        );
                    }
                    ::spel_katalog_games::Request::FindImages { slugs } => {
                        return self
                            .image_buffer
                            .find_images(slugs, self.settings.get::<CoverartDir>().to_path_buf())
                            .map(OrRequest::Message)
                            .map(Message::Games);
                    }
                    ::spel_katalog_games::Request::CloseInfo => {
                        self.view.show_info(false);
                        self.games.select(SelDir::None);
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
                        self.view.show_info(show);
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
                        return self.run_game(id, Safety::from(sandbox), false);
                    }
                    ::spel_katalog_info::Request::RunLutrisInSandbox { id } => {
                        return self.run_game(id, Safety::Firejail, true);
                    }
                }
            }
            Message::RunGame(id, safety) => return self.run_game(id, safety, false),
            Message::Quick(quick) => match quick {
                QuickMessage::CloseAll => {
                    self.process_list = None;
                    self.view.show_info(false);
                    self.view.show_settings(false);
                    self.games.select(SelDir::None);
                    self.filter = String::new();
                    self.sort_games();
                }
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
                    } else if self.games.selected().is_some() {
                        self.games.select(SelDir::None);
                    } else if !self.filter.is_empty() {
                        self.filter = String::new();
                        self.sort_games();
                    }
                }
                QuickMessage::ToggleSettings => {
                    self.view.show_settings(!self.view.settings_shown());
                }
                QuickMessage::OpenProcessInfo => {
                    return Task::future(Self::collect_process_info()).then(identity);
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
                    self.set_status(format!("toggled network to {next}"));
                    self.sort_games();
                }
                QuickMessage::RefreshProcessInfo => {
                    if self.process_list.is_some() {
                        return Task::future(Self::collect_process_info()).then(identity);
                    }
                }
                QuickMessage::Next => return widget::focus_next(),
                QuickMessage::Prev => return widget::focus_previous(),
                QuickMessage::RunSelected => {
                    if let Some(id) = self.games.selected() {
                        return self.run_game(id, Safety::Firejail, false);
                    }
                }
                QuickMessage::ToggleBatch => self.show_batch = !self.show_batch,
            },
            Message::ProcessInfo(process_infos) => {
                self.process_list = process_infos.filter(|infos| !infos.is_empty())
            }
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
                })
                .then(|_| Task::none());
            }
            Message::Batch(or_request) => match or_request {
                OrRequest::Message(msg) => {
                    return self
                        .batch
                        .update(msg, &self.sender, &self.settings)
                        .map(From::from);
                }
                OrRequest::Request(req) => match req {
                    ::spel_katalog_batch::Request::ShowProcesses => {
                        return Task::done(Message::Quick(QuickMessage::OpenProcessInfo));
                    }
                    ::spel_katalog_batch::Request::HideBatch => self.show_batch = false,
                    ::spel_katalog_batch::Request::GatherBatchInfo(scope) => {
                        fn gather<'a>(
                            yml_dir: &str,
                            games: impl IntoIterator<Item = &'a ::spel_katalog_games::Game>,
                        ) -> Task<Message> {
                            games
                                .into_iter()
                                .map(|game| BatchInfo {
                                    id: game.id,
                                    slug: game.slug.clone(),
                                    name: game.name.clone(),
                                    runner: game.runner.to_string(),
                                    config: format!("{yml_dir}/{}.yml", game.configpath),
                                    hidden: game.hidden,
                                })
                                .collect::<Vec<_>>()
                                .pipe(::spel_katalog_batch::Message::RunBatch)
                                .pipe(OrRequest::Message)
                                .pipe(Message::Batch)
                                .pipe(Task::done)
                        }
                        let yml_dir = self.settings.get::<YmlDir>();
                        let yml_dir = yml_dir.as_str();
                        return match scope {
                            ::spel_katalog_batch::Scope::All => gather(yml_dir, self.games.all()),
                            ::spel_katalog_batch::Scope::Shown => {
                                gather(yml_dir, self.games.displayed())
                            }
                            ::spel_katalog_batch::Scope::Batch => {
                                gather(yml_dir, self.games.batch_selected())
                            }
                        };
                    }
                    ::spel_katalog_batch::Request::ReloadCache => {
                        return self.games.find_cached(&self.settings).map(Message::Games);
                    }
                },
            },
        }
        Task::none()
    }

    pub async fn collect_process_info() -> Task<Message> {
        match process_info::ProcessInfo::open().await {
            Ok(summary) => Task::done(Message::ProcessInfo(Some(summary))),
            Err(err) => {
                ::log::error!("whilst collecting info\n{err}");
                Task::none()
            }
        }
    }

    pub fn sort_games(&mut self) {
        self.games.sort(&self.settings, &self.filter);
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        let status = status.into();
        ::log::info!("status: {status}");
        self.status = status;
    }

    fn run_game(&mut self, id: i64, safety: Safety, no_game: bool) -> Task<Message> {
        let Some(game) = self.games.by_id(id) else {
            status!(&self.sender, "could not run game with id {id}");
            return Task::none();
        };

        let lutris = self.settings.get::<LutrisExe>().clone();
        let firejail = self.settings.get::<FirejailExe>().clone();
        let slug = game.slug.clone();
        let name = game.name.clone();
        let runner = game.runner.clone();
        let hidden = game.hidden;
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
        let script_dir = self.settings.get::<ScriptConfigDir>().to_path_buf();

        let (send_open, recv_open) = ::tokio::sync::oneshot::channel();

        let open_process_list = Task::future(async {
            match recv_open.await {
                Ok(_) => Some(Message::Quick(QuickMessage::OpenProcessInfo)),
                Err(err) => {
                    ::log::error!("could not receive open signel through oneshot\n{err}");
                    None
                }
            }
        })
        .then(|msg| msg.map_or_else(Task::none, Task::done));

        let task = Task::future(async move {
            #[derive(Debug, ::thiserror::Error)]
            enum ScriptGatherError {
                /// Forwarded io error.
                #[error(transparent)]
                Io(#[from] ::std::io::Error),
                /// Forwarded script run error.
                #[error(transparent)]
                Script(#[from] ::spel_katalog_script::Error),
                /// Forwarded parse error.
                #[error(transparent)]
                ParseRead(#[from] ::spel_katalog_script::ReadError),
                /// Forwarded string interpolation error.
                #[error("while interpolating {1:?}\n{0}")]
                Interpolate(
                    #[source] ::spel_katalog_parse::InterpolationError<String>,
                    String,
                ),
            }

            #[derive(Debug, ::thiserror::Error)]
            enum ConfigError {
                #[error(transparent)]
                Io(#[from] ::std::io::Error),
                #[error(transparent)]
                Scan(#[from] ::yaml_rust2::ScanError),
            }

            let config = async {
                let config = ::tokio::fs::read_to_string(&configpath).await?;
                let config = formats::Config::parse(&config)?;

                Ok::<_, ConfigError>(config)
            };

            let config = match config.await {
                Ok(config) => config,
                Err(err) => {
                    ::log::error!("while loading config {configpath:?}\n{err}");
                    return format!("could not load config for game").into();
                }
            };

            let extra_config = if extra_config_path.exists() {
                let Some(content) = ::tokio::fs::read_to_string(&extra_config_path)
                    .await
                    .map_err(|err| ::log::error!("could not read {extra_config_path:?}\n{err}"))
                    .ok()
                else {
                    return format!("could not read {extra_config_path:?}").into();
                };
                let Some(additional) = ::toml::from_str::<Additional>(&content)
                    .map_err(|err| ::log::error!("could not parse {extra_config_path:?}\n{err}"))
                    .ok()
                else {
                    return format!("could not parse {extra_config_path:?}").into();
                };

                Some(additional)
            } else {
                None
            };

            let scripts_result = async {
                if !script_dir.exists() {
                    ::log::info!("no script dir, skipping");
                    return Ok(());
                }

                let mut scripts = Vec::new();
                let mut stack = Vec::new();

                stack.push(script_dir);

                while let Some(dir) = stack.pop() {
                    let mut dir = ::tokio::fs::read_dir(dir).await?;
                    while let Some(entry) = dir.next_entry().await? {
                        let ft = entry.file_type().await?;

                        let path = entry.path();

                        if ft.is_dir() {
                            stack.push(path);
                        } else if ft.is_file() {
                            scripts.push(path);
                        } else {
                            ::log::warn!("non file or directory path in script dir, {path:?}")
                        }
                    }
                }

                scripts.sort_unstable();

                let mut scripts = scripts
                    .iter()
                    .map(|path| ScriptFile::read(&path))
                    .collect::<FuturesOrdered<_>>()
                    .try_collect::<Vec<_>>()
                    .await?;

                for script in &mut scripts {
                    let globals = mem::take(&mut script.global);
                    script
                        .visit_strings(|s| {
                            *s = ::spel_katalog_parse::interpolate_string(s, |key| match key {
                                "HOME" => Some(::spel_katalog_settings::HOME.to_string()),
                                "ID" => Some(id.to_string()),
                                "SLUG" => Some(slug.clone()),
                                "NAME" => Some(name.clone()),
                                "RUNNER" => Some(runner.to_string()),
                                "HIDDEN" => Some(hidden.to_string()),
                                "EXE" => Some(config.game.exe.display().to_string()),
                                "PREFIX" => Some(config.game.prefix.as_ref().map_or_else(
                                    || String::new(),
                                    |pfx| pfx.display().to_string(),
                                )),
                                "ARCH" => Some(config.game.arch.clone().unwrap_or_default()),
                                key => {
                                    if let Some(global) = key.strip_prefix("GLOBAL.")
                                        && let Some(value) = globals.get(global)
                                    {
                                        value.clone().pipe(Some)
                                    } else if let Some(attr) = key.strip_prefix("ATTR.") {
                                        extra_config
                                            .as_ref()
                                            .and_then(|extra_config| {
                                                extra_config.attrs.get(attr).cloned()
                                            })
                                            .unwrap_or_default()
                                            .pipe(Some)
                                    } else {
                                        None
                                    }
                                }
                            })?;
                            Ok(())
                        })
                        .map_err(|err| {
                            ScriptGatherError::Interpolate(
                                err,
                                script.source.as_ref().map_or_else(
                                    || script.id().to_owned(),
                                    |source| source.display().to_string(),
                                ),
                            )
                        })?;
                }

                ScriptFile::run_all(&scripts).await?;
                Ok::<_, ScriptGatherError>(())
            };

            if let Err(err) = scripts_result.await {
                ::log::error!("failure when gathering/runnings scripts\n{err}");
                return format!("running scripts failed").into();
            }

            let rungame = if no_game {
                None
            } else {
                Some(format!("lutris:rungameid/{id}"))
            };

            fn wl(p: impl AsRef<OsStr>) -> OsString {
                let mut s = OsString::new();
                s.push("--whitelist=");
                s.push(p);
                s
            }

            let cmd = match safety {
                Safety::None => {
                    ::log::info!("executing {lutris:?} with arguments\n{:#?}", &[&rungame]);
                    ::tokio::process::Command::new(lutris)
                        .args(rungame)
                        .kill_on_drop(true)
                        .status()
                }
                Safety::Firejail => {
                    let mut args = Vec::new();
                    ::log::info!("parsed game config\n{config:#?}");
                    if let Some(additional) = extra_config
                        .map(|additional| additional.sandbox_root)
                        .filter(|roots| !roots.is_empty())
                    {
                        args.extend(additional.into_iter().map(wl));
                    } else {
                        args.push(wl(config.game.common_parent()));
                    }

                    if is_net_disabled {
                        args.push("--net=none".into());
                    }

                    args.push(lutris.as_os_str().into());

                    if let Some(rungame) = rungame {
                        args.push(rungame.into());
                    };

                    ::log::info!("executing {firejail:?} with arguments\n{args:#?}");

                    ::tokio::process::Command::new(firejail)
                        .args(args)
                        .kill_on_drop(true)
                        .status()
                }
            };

            if send_open.send(()).is_err() {
                ::log::warn!("could not send open signal through oneshot");
            }

            match cmd.await {
                Ok(status) => format!("{name} exited with {status}").into(),
                Err(err) => {
                    ::log::error!("could not run {slug}\n{err}");
                    format!("could not run {slug}").into()
                }
            }
        });

        Task::batch([task, open_process_list])
    }

    pub fn view(&self) -> Element<'_, Message> {
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
            .push(
                stack([self
                    .view
                    .view(
                        &self.settings,
                        &self.games,
                        &self.info,
                        self.process_list.is_some() || self.show_batch,
                    )
                    .into()])
                .push_maybe(self.show_batch.then(|| {
                    self.batch
                        .view()
                        .pipe(widget::container)
                        .padding(50)
                        .center_x(Fill)
                        .height(Fill)
                        .pipe(widget::opaque)
                        .pipe(Element::from)
                        .map(Message::Batch)
                }))
                .push_maybe(
                    self.process_list
                        .as_ref()
                        .map(|process_info| process_info::ProcessInfo::view_list(&process_info)),
                ),
            )
            .push(vertical_space().height(3))
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
