use ::std::{
    convert::identity,
    io::{BufWriter, Write},
    path::Path,
    sync::Arc,
    time::Duration,
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
use ::spel_katalog_common::{
    OrRequest, StatusSender,
    tracker::{self, create_tracker_monitor},
    w,
};
use ::spel_katalog_info::image_buffer::ImageBuffer;
use ::spel_katalog_settings::{ConfigDir, FilterMode, LutrisDb, Network, Theme};
use ::spel_katalog_sink::SinkBuilder;
use ::tap::Pipe;
use ::tokio::sync::mpsc::{Receiver, Sender, channel};
use ::tokio_stream::wrappers::ReceiverStream;

use crate::{
    Cli, ExitReceiver, Message,
    cli::Subcmd,
    dialog::{Dialog, DialogBuilder},
    init_config::init_config,
    process_info, view,
};

/// Specific kind of window.
#[derive(Debug, Clone)]
pub enum WindowType {
    /// Window is the main window.
    Main,
    /// Show lua api.
    LuaApi,
    /// Show a dialog window.
    Dialog(Dialog),
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
    pub api_markdown: Box<[widget::markdown::Item]>,
    pub lua_vt: Arc<LuaVt>,
    pub batch_source: Option<String>,
    pub batch_init_timeout: u16,
}

/// Virtual table passed to lua.
#[derive(Debug, Clone)]
pub struct LuaVt {
    sender: Sender<DialogBuilder>,
    lib_dir: Arc<Path>,
}

impl LuaVt {
    fn new(lib_dir: Arc<Path>) -> (Self, Receiver<DialogBuilder>) {
        let (sender, rx) = channel(64);
        (Self { sender, lib_dir }, rx)
    }
}

impl ::spel_katalog_lua::Virtual for LuaVt {
    fn available_modules(&self) -> FxHashMap<String, String> {
        let lib_dir = AsRef::<Path>::as_ref(&self.lib_dir);
        ::std::fs::read_dir(lib_dir)
            .map_err(|err| ::log::error!("could not read directory {lib_dir:?}\n{err}"))
            .ok()
            .into_iter()
            .flat_map(|read_dir| {
                read_dir.filter_map(|entry| {
                    let entry = entry
                        .map_err(|err| {
                            ::log::error!("failed to get read_dir entry in {lib_dir:?}\n{err}")
                        })
                        .ok()?;

                    let path = entry.path();
                    let name = path.file_stem();
                    if name.is_none() {
                        ::log::error!("could not get file stem for {path:?}")
                    }
                    let name = name?;
                    let content = ::std::fs::read_to_string(&path)
                        .map_err(|err| {
                            ::log::error!("could not read content of {path:?} to a string\n{err}");
                        })
                        .ok()?;

                    Some((name.to_string_lossy().into_owned(), content))
                })
            })
            .collect()
    }

    fn open_dialog(&self, text: String, buttons: Vec<String>) -> mlua::Result<Option<String>> {
        let (dialog, mut rx) = DialogBuilder::new(text, buttons);
        self.sender
            .blocking_send(dialog)
            .map_err(::mlua::Error::external)?;
        Ok(rx.blocking_recv())
    }
}

impl App {
    fn new(
        cli: Cli,
        sink_builder: SinkBuilder,
    ) -> ::color_eyre::Result<(Self, Receiver<String>, Receiver<DialogBuilder>)> {
        let Cli {
            settings,
            show_settings,
            config,
            action,
            advanced_terminal: _,
            keep_terminal: _,
            batch,
            batch_init_timeout,
        } = cli;

        let batch_source = batch
            .map(::std::fs::read_to_string)
            .transpose()
            .map_err(|err| eyre!("could not read given batch script to a string").wrap_err(err))?;

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
                Subcmd::InitConfig {
                    path,
                    skip_lua_update,
                } => {
                    init_config(path, skip_lua_update);
                    ::std::process::exit(0)
                }
            }
        }

        let (status_tx, status_rx) = ::tokio::sync::mpsc::channel(64);

        let filter = String::new();
        let status = String::new();
        let view = view::State::new(show_settings);
        let settings = ::spel_katalog_settings::State { settings, config };
        let games = ::spel_katalog_games::State::default();
        let image_buffer = ImageBuffer::empty();
        let info = ::spel_katalog_info::State::default();
        let sender = status_tx.into();
        let process_list = None;
        let show_batch = false;
        let windows = FxHashMap::default();
        let batch = Default::default();
        let lib_path = Arc::from(settings.get::<ConfigDir>().as_path().join("lib"));
        let api_markdown = widget::markdown::parse(include_str!("../../lua/docs.md")).collect();
        let (lua_vt, dialog_rx) = LuaVt::new(lib_path);
        let lua_vt = Arc::new(lua_vt);

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
                api_markdown,
                lua_vt,
                batch_source,
                batch_init_timeout,
            },
            status_rx,
            dialog_rx,
        ))
    }

    pub fn run(
        cli: Cli,
        sink_builder: SinkBuilder,
        exit_recv: Option<ExitReceiver>,
    ) -> ::color_eyre::Result<()> {
        let (app, status_rx, dialog_rx) = Self::new(cli, sink_builder)?;

        let (tracker, run_batch) = if app.batch_source.is_some() {
            let (tracker, monitor) = create_tracker_monitor();

            let timeout = Duration::from_secs(app.batch_init_timeout.into());
            let run_batch = Task::future(async move {
                match ::tokio::time::timeout(timeout, monitor.wait()).await {
                    Ok(response) => response,
                    Err(err) => {
                        ::log::warn!(
                            "initialization did not finish before timout {timeout}, {err}",
                            timeout = timeout.as_secs()
                        );
                        tracker::Response::Lost
                    }
                }
            })
            .then(move |response| {
                if !response.is_finished() {
                    ::log::warn!("initialization tracker was lost");
                }

                Task::done(Message::BatchRun)
            });

            (Some(tracker), Some(run_batch))
        } else {
            (None, None)
        };

        let main = if let Some(run_batch) = run_batch {
            run_batch
        } else {
            let (_, open_main) = window::open(::iced::window::Settings::default());
            open_main.map(|id| Message::OpenWindow(id, WindowType::Main))
        };

        ::iced::daemon("Lutris Games", Self::update, Self::view)
            .theme(|app, _| ::iced::Theme::from(*app.settings.settings.get::<Theme>()))
            .subscription(Self::subscription)
            .executor::<::tokio::runtime::Runtime>()
            .run_with(move || {
                let load_db = app
                    .settings
                    .get::<LutrisDb>()
                    .to_path_buf()
                    .pipe(move |db_path| spel_katalog_games::Message::LoadDb { db_path, tracker })
                    .pipe(OrRequest::Message)
                    .pipe(Message::Games)
                    .pipe(Task::done);
                let receive_status =
                    Task::stream(ReceiverStream::new(status_rx)).map(Message::Status);
                let receive_dialog =
                    Task::stream(ReceiverStream::new(dialog_rx)).map(Message::Dialog);
                let exit_recv = exit_recv
                    .map(|exit_recv| Task::future(exit_recv.recv()).then(|_| ::iced::exit()))
                    .unwrap_or_else(Task::none);

                let batch = Task::batch([receive_status, receive_dialog, load_db, main, exit_recv]);

                (app, batch)
            })
            .map_err(Report::from)
    }

    pub fn lua_vt(&self) -> Arc<LuaVt> {
        self.lua_vt.clone()
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
            return widget::container("No Window Type").center(Fill).into();
        };

        match ty {
            WindowType::Main => self.view_main(),
            WindowType::LuaApi => crate::api_window::view(
                &self.api_markdown,
                (*self.settings.get::<Theme>())
                    .pipe(::iced::Theme::from)
                    .palette(),
            )
            .map(Message::Url),
            WindowType::Dialog(dialog) => dialog
                .view()
                .map(move |msg| Message::DialogMessage(id, msg)),
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
                .on_input(identity)
                .pipe(Element::from)
                .map(Message::Filter),
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
