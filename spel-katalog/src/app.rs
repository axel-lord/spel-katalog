use ::std::{collections::HashMap, convert::identity, io::PipeReader, path::PathBuf, sync::Arc};

use ::color_eyre::Report;
use ::derive_more::IsVariant;
use ::iced::{
    Element,
    Length::Fill,
    Task,
    widget::{self, horizontal_rule, stack, text, text_input, toggler, value, vertical_space},
    window,
};
use ::rustc_hash::FxHashMap;
use ::spel_katalog_cli::Run;
use ::spel_katalog_common::{OrRequest, StatusSender, w};
use ::spel_katalog_settings::{CacheDir, ConfigDir, FilterMode, LutrisDb, Network, Theme};
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};
use ::tap::Pipe;
use ::tokio::sync::mpsc::{Receiver, Sender, channel};
use ::tokio_stream::wrappers::ReceiverStream;

use crate::{
    ExitReceiver, Message,
    dialog::{Dialog, DialogBuilder},
    get_modules, get_settings, process_info, view,
};

/// Specific kind of window.
#[derive(Debug, IsVariant)]
pub enum WindowType {
    /// Window is the main window.
    Main,
    /// Show lua api.
    LuaApi,
    /// Show terminal.
    Term,
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
    pub sender: StatusSender,
    pub process_list: Option<Vec<process_info::ProcessInfo>>,
    pub sink_builder: SinkBuilder,
    pub windows: FxHashMap<window::Id, WindowType>,
    pub dialog_tx: Sender<DialogBuilder>,
    pub terminal: ::spel_katalog_terminal::Terminal,
    pub docs_viewer: ::spel_katalog_lua_docs::DocsViewer,
}

/// Virtual table passed to lua.
#[derive(Debug)]
pub struct LuaVt {
    pub sender: Sender<DialogBuilder>,
    pub settings: ::spel_katalog_settings::Settings,
}

impl ::spel_katalog_lua::Virtual for LuaVt {
    fn available_modules(&self) -> FxHashMap<String, String> {
        get_modules(&self.settings)
    }

    fn open_dialog(&self, text: String, buttons: Vec<String>) -> mlua::Result<Option<String>> {
        let (dialog, mut rx) = DialogBuilder::new(text, buttons);
        self.sender
            .blocking_send(dialog)
            .map_err(::mlua::Error::external)?;
        Ok(rx.blocking_recv())
    }

    fn thumb_db_path(&self) -> mlua::Result<PathBuf> {
        Ok(self
            .settings
            .get::<CacheDir>()
            .as_path()
            .join("thumbnails.db"))
    }

    fn settings(&self) -> mlua::Result<HashMap<&'_ str, String>> {
        Ok(self.settings.generic())
    }

    fn additional_config_path(&self, game_id: i64) -> mlua::Result<PathBuf> {
        Ok(self
            .settings
            .get::<ConfigDir>()
            .as_path()
            .join(format!("games/{game_id}.toml")))
    }
}

/// Initial state created by new.
#[derive(Debug)]
struct Initial {
    app: App,
    status_rx: Receiver<String>,
    dialog_rx: Receiver<DialogBuilder>,
    terminal_rx: Option<::std::sync::mpsc::Receiver<(PipeReader, SinkIdentity)>>,
}

impl Initial {
    fn new(run: Run, sink_builder: SinkBuilder) -> ::color_eyre::Result<Self> {
        let Run {
            advanced_terminal: _,
            config,
            keep_terminal: _,
            settings,
            show_settings,
            show_terminal,
        } = run;

        let settings = get_settings(&config, settings);

        let (status_tx, status_rx) = ::tokio::sync::mpsc::channel(64);

        let filter = String::new();
        let status = String::new();
        let view = view::State::new(show_settings);
        let settings = ::spel_katalog_settings::State { settings, config };
        let games = ::spel_katalog_games::State::default();
        let info = ::spel_katalog_info::State::default();
        let sender = status_tx.into();
        let process_list = None;
        let show_batch = false;
        let windows = FxHashMap::default();
        let batch = Default::default();
        let (dialog_tx, dialog_rx) = channel(64);
        let terminal = ::spel_katalog_terminal::Terminal::default().with_limit(256);
        let docs_viewer = Default::default();

        let (sink_builder, terminal_rx) = if show_terminal {
            let (terminal_tx, terminal_rx) = ::std::sync::mpsc::channel();
            (SinkBuilder::CreatePipe(terminal_tx), Some(terminal_rx))
        } else {
            (sink_builder, None)
        };

        let app = App {
            batch,
            dialog_tx,
            filter,
            games,
            info,
            process_list,
            sender,
            settings,
            show_batch,
            sink_builder,
            status,
            terminal,
            view,
            windows,
            docs_viewer,
        };

        Ok(Self {
            app,
            dialog_rx,
            status_rx,
            terminal_rx,
        })
    }
}

impl App {
    pub fn run(
        run: Run,
        sink_builder: SinkBuilder,
        exit_recv: Option<ExitReceiver>,
    ) -> ::color_eyre::Result<()> {
        let Initial {
            app,
            status_rx,
            dialog_rx,
            terminal_rx,
        } = Initial::new(run, sink_builder)?;

        ::iced::daemon("Lutris Games", Self::update, Self::view)
            .theme(|app, _| ::iced::Theme::from(*app.settings.settings.get::<Theme>()))
            .subscription(Self::subscription)
            .executor::<::tokio::runtime::Runtime>()
            .run_with(move || {
                let (_, open_main) = window::open(::iced::window::Settings::default());
                let main = open_main.map(|id| Message::OpenWindow(id, WindowType::Main));
                let load_db = app
                    .settings
                    .get::<LutrisDb>()
                    .to_path_buf()
                    .pipe(move |db_path| spel_katalog_games::Message::LoadDb { db_path })
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
                let window_recv = terminal_rx
                    .map(|terminal_rx| {
                        let (_, task) = window::open(Default::default());
                        Task::batch([
                            ::spel_katalog_terminal::Message::sink_receiver(terminal_rx)
                                .map(Message::from),
                            task.map(|id| Message::OpenWindow(id, WindowType::Term)),
                        ])
                    })
                    .unwrap_or_else(Task::none);

                let batch = Task::batch([
                    receive_status,
                    receive_dialog,
                    load_db,
                    main,
                    exit_recv,
                    window_recv,
                ]);

                (app, batch)
            })
            .map_err(Report::from)
    }

    pub fn lua_vt(&self) -> Arc<LuaVt> {
        Arc::new(LuaVt {
            sender: self.dialog_tx.clone(),
            settings: self.settings.settings.clone(),
        })
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
            WindowType::LuaApi => self.docs_viewer.view().map(Message::LuaDocs),
            WindowType::Dialog(dialog) => dialog
                .view()
                .map(move |msg| Message::DialogMessage(id, msg)),
            WindowType::Term => self.terminal.view().map(From::from),
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
