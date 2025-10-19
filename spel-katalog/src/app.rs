use ::std::{
    io::{BufWriter, Write},
    path::Path,
};

use ::clap::CommandFactory;
use ::color_eyre::{Report, Section, eyre::eyre};
use ::iced::{
    Element,
    Length::Fill,
    Task,
    widget::{self, horizontal_rule, stack, text, text_input, toggler, value, vertical_space},
    window,
};
use ::rustc_hash::FxHashMap;
use ::spel_katalog_common::{OrRequest, StatusSender, w};
use ::spel_katalog_info::image_buffer::ImageBuffer;
use ::spel_katalog_settings::{FilterMode, LutrisDb, Network, Theme};
use ::spel_katalog_sink::SinkBuilder;
use ::tap::Pipe;

use crate::{Cli, ExitReceiver, Message, cli::Subcmd, process_info, view};

/// Specific kind of window.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum WindowType {
    /// Window is the main window.
    Main,
}

#[derive(Debug)]
pub(crate) struct App {
    pub settings: ::spel_katalog_settings::State,
    pub games: ::spel_katalog_games::State,
    pub status: String,
    pub filter: String,
    pub view: view::State,
    pub info: ::spel_katalog_info::State,
    pub batch: ::spel_katalog_batch::State,
    pub show_batch: bool,
    pub image_buffer: ImageBuffer,
    pub sender: StatusSender,
    pub process_list: Option<Vec<process_info::ProcessInfo>>,
    pub sink_builder: SinkBuilder,
    pub windows: FxHashMap<window::Id, WindowType>,
}

impl App {
    fn new(
        cli: Cli,
        sink_builder: SinkBuilder,
    ) -> ::color_eyre::Result<(Self, ::tokio::sync::mpsc::Receiver<String>)> {
        let Cli {
            settings,
            show_settings,
            config,
            action,
            advanced_terminal: _,
            keep_terminal: _,
        } = cli;

        fn read_settings(path: &Path) -> ::color_eyre::Result<::spel_katalog_settings::Settings> {
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
                            .map_err(|err| eyre!("could not create/open {output:?}").error(err))?;
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
                            .map_err(|err| eyre!("could not create/open {output:?}").error(err))?;
                        ::clap_complete::generate(shell, &mut Cli::command(), name, &mut writer);
                    }
                    ::std::process::exit(0)
                }
            }
        }

        let (tx, rx) = ::tokio::sync::mpsc::channel(64);

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
        let windows = FxHashMap::default();
        let batch = Default::default();

        Ok((
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
                windows,
            },
            rx,
        ))
    }

    pub fn run(
        cli: Cli,
        sink_builder: SinkBuilder,
        exit_recv: Option<ExitReceiver>,
    ) -> ::color_eyre::Result<()> {
        let (app, rx) = Self::new(cli, sink_builder)?;

        ::iced::daemon("Lutris Games", Self::update, Self::view)
            .theme(|app, _| ::iced::Theme::from(*app.settings.settings.get::<Theme>()))
            .subscription(Self::subscription)
            .executor::<::tokio::runtime::Runtime>()
            .run_with(move || {
                let load_db = app
                    .settings
                    .get::<LutrisDb>()
                    .to_path_buf()
                    .pipe(spel_katalog_games::Message::LoadDb)
                    .pipe(OrRequest::Message)
                    .pipe(Message::Games)
                    .pipe(Task::done);
                let receive_status =
                    Task::stream(::tokio_stream::wrappers::ReceiverStream::new(rx))
                        .map(Message::Status);
                let (_, open_main) = window::open(::iced::window::Settings::default());
                let open_main = open_main.map(|id| Message::OpenWindow(id, WindowType::Main));
                let exit_recv = exit_recv
                    .map(|exit_recv| Task::future(exit_recv.recv()).then(|_| ::iced::exit()));

                let batch = Task::batch(
                    [receive_status, load_db, open_main]
                        .into_iter()
                        .chain(exit_recv),
                );

                (app, batch)
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

    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        let Some(ty) = self.windows.get(&id) else {
            return widget::Space::new(10, 10).into();
        };

        match ty {
            WindowType::Main => self.view_main(),
        }
    }

    pub fn view_main(&self) -> Element<'_, Message> {
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
