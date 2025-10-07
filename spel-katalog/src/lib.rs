use ::std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use ::clap::{CommandFactory, Parser, Subcommand};
use ::color_eyre::{Report, Section, eyre::eyre};
use ::derive_more::IsVariant;
use ::iced::{
    Element,
    Length::Fill,
    Task,
    widget::{self, horizontal_rule, stack, text, text_input, toggler, value, vertical_space},
};
use ::spel_katalog_common::{OrRequest, StatusSender, w};
use ::spel_katalog_info::image_buffer::ImageBuffer;
use ::spel_katalog_settings::{FilterMode, LutrisDb, Network, Theme};
use ::spel_katalog_terminal::SinkBuilder;
use ::tap::Pipe;

pub use self::{
    exit_channel::{ExitReceiver, ExitSender, exit_channel},
    message::{Message, QuickMessage},
};

mod exit_channel;
mod message;
mod process_info;
mod run_game;
mod subscription;
mod update;
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

    /// Advanced terminal output.
    #[arg(long, visible_alias = "at")]
    pub advanced_terminal: bool,

    /// Keep terminal open.
    #[arg(long, visible_alias = "kt", requires("advanced_terminal"))]
    pub keep_terminal: bool,

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
    sink_builder: SinkBuilder,
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

impl App {
    pub fn run(
        cli: Cli,
        sink_builder: SinkBuilder,
        exit_recv: Option<ExitReceiver>,
    ) -> ::color_eyre::Result<()> {
        let rx;
        let app = {
            let Cli {
                settings,
                show_settings,
                config,
                action,
                advanced_terminal: _,
                keep_terminal: _,
            } = cli;

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
                sink_builder,
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

                (
                    app,
                    Task::batch([receive_status, load_db].into_iter().chain(
                        exit_recv.map(|exit_recv| {
                            Task::future(exit_recv.recv()).then(|_| ::iced::exit())
                        }),
                    )),
                )
            })
            .map_err(Report::from)
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
