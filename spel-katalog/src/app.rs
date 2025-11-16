use ::std::{collections::HashMap, convert::identity, io::PipeReader, path::PathBuf, sync::Arc};

use ::derive_more::IsVariant;
use ::iced_core::{Alignment::Center, Length::Fill, window};
use ::iced_runtime::Task;
use ::iced_widget::{
    self as widget, Row, horizontal_rule, horizontal_space, text, text_input, toggler, value,
    vertical_space,
};
use ::rustc_hash::FxHashMap;
use ::spel_katalog_cli::Run;
use ::spel_katalog_common::{OrRequest, StatusSender, w};
use ::spel_katalog_settings::{CacheDir, ConfigDir, FilterMode, LutrisDb, Network, Theme};
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};
use ::tap::Pipe;

use crate::{
    Element, ExitReceiver, Message, QuickMessage,
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
    /// Show a settings window.
    Settings,
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
    pub sender: StatusSender,
    pub process_list: Vec<process_info::ProcessInfo>,
    pub sink_builder: SinkBuilder,
    pub windows: FxHashMap<window::Id, WindowType>,
    pub dialog_tx: ::flume::Sender<DialogBuilder>,
    pub terminal: ::spel_katalog_terminal::Terminal,
    pub docs_viewer: ::spel_katalog_lua_docs::DocsViewer,
}

/// Virtual table passed to lua.
#[derive(Debug)]
pub struct LuaVt {
    pub sender: ::flume::Sender<DialogBuilder>,
    pub settings: ::spel_katalog_settings::Settings,
}

impl ::spel_katalog_lua::Virtual for LuaVt {
    fn available_modules(&self) -> FxHashMap<String, String> {
        get_modules(&self.settings)
    }

    fn open_dialog(&self, text: String, buttons: Vec<String>) -> mlua::Result<Option<String>> {
        let (dialog, rx) = DialogBuilder::new(text, buttons);
        self.sender.send(dialog).map_err(::mlua::Error::external)?;
        Ok(rx.recv().ok())
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
    status_rx: ::flume::Receiver<String>,
    dialog_rx: ::flume::Receiver<DialogBuilder>,
    terminal_rx: Option<::flume::Receiver<(PipeReader, SinkIdentity)>>,
    show_settings: bool,
}

/// Flags used to start application.
#[derive(Debug)]
pub struct Flags {
    /// Initial state.
    initial: Initial,
    /// Exit receiver.
    exit_recv: Option<ExitReceiver>,
}

impl Initial {
    fn new(run: Run, sink_builder: SinkBuilder) -> ::color_eyre::Result<Self> {
        let Run {
            config,
            keep_terminal: _,
            settings,
            show_settings,
            show_terminal,
        } = run;

        let settings = get_settings(&config, settings);

        let (status_tx, status_rx) = ::flume::bounded(64);

        let filter = String::new();
        let status = String::new();
        let view = view::State::new();
        let settings = ::spel_katalog_settings::State { settings, config };
        let games = ::spel_katalog_games::State::default();
        let info = ::spel_katalog_info::State::default();
        let sender = status_tx.into();
        let process_list = Vec::new();
        let windows = FxHashMap::default();
        let batch = Default::default();
        let (dialog_tx, dialog_rx) = ::flume::bounded(64);
        let terminal = ::spel_katalog_terminal::Terminal::default().with_limit(256);
        let docs_viewer = Default::default();

        let (sink_builder, terminal_rx) = if show_terminal {
            let (terminal_tx, terminal_rx) = ::flume::unbounded();
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
            show_settings,
        })
    }
}

impl ::iced_winit::Program for App {
    type Message = Message;

    type Theme = ::iced_core::Theme;

    type Executor = ::iced_futures::backend::default::Executor;

    type Renderer = ::iced_renderer::Renderer;

    type Flags = Flags;

    fn new(
        Flags {
            initial:
                Initial {
                    app,
                    status_rx,
                    dialog_rx,
                    terminal_rx,
                    show_settings,
                },
            exit_recv,
        }: Self::Flags,
    ) -> (Self, Task<Self::Message>) {
        let (_, open_main) = ::iced_runtime::window::open(::iced_core::window::Settings::default());
        let main = open_main.map(|id| Message::OpenWindow(id, WindowType::Main));
        let load_db = app
            .settings
            .get::<LutrisDb>()
            .to_path_buf()
            .pipe(move |db_path| spel_katalog_games::Message::LoadDb { db_path })
            .pipe(OrRequest::Message)
            .pipe(Message::Games)
            .pipe(Task::done);
        let receive_status = Task::stream(status_rx.into_stream()).map(Message::Status);
        let receive_dialog = Task::stream(dialog_rx.into_stream()).map(Message::BuildDialog);
        let exit_recv = exit_recv
            .map(|exit_recv| Task::future(exit_recv.recv()).then(|_| ::iced_runtime::exit()))
            .unwrap_or_else(Task::none);
        let window_recv = terminal_rx
            .map(|terminal_rx| {
                let (_, task) = ::iced_runtime::window::open(Default::default());
                Task::batch([
                    ::spel_katalog_terminal::Message::sink_receiver(terminal_rx).map(Message::from),
                    task.map(|id| Message::OpenWindow(id, WindowType::Term)),
                ])
            })
            .unwrap_or_else(Task::none);
        let show_settings = if show_settings {
            Task::done(Message::Quick(QuickMessage::ToggleSettings))
        } else {
            Task::none()
        };

        let batch = Task::batch([
            receive_status,
            receive_dialog,
            load_db,
            main,
            exit_recv,
            window_recv,
            show_settings,
        ]);

        (app, batch)
    }

    fn title(&self, _window: window::Id) -> String {
        "Lutris Games".to_owned()
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        self.update(message)
    }

    fn view(
        &self,
        window: window::Id,
    ) -> iced_core::Element<'_, Self::Message, Self::Theme, Self::Renderer> {
        self.view(window)
    }

    fn subscription(&self) -> iced_futures::Subscription<Self::Message> {
        self.subscription()
    }

    fn theme(&self, _window: window::Id) -> Self::Theme {
        ::iced_core::Theme::from(*self.settings.get::<Theme>())
    }
}

impl App {
    pub fn run(
        run: Run,
        sink_builder: SinkBuilder,
        exit_recv: Option<ExitReceiver>,
    ) -> ::color_eyre::Result<()> {
        ::iced_winit::program::run::<Self, ::iced_renderer::Compositor>(
            Default::default(),
            Default::default(),
            Default::default(),
            Flags {
                initial: Initial::new(run, sink_builder)?,
                exit_recv,
            },
        )
        .map_err(|err| ::color_eyre::eyre::eyre!(err))
    }

    pub fn lua_vt(&self) -> Arc<LuaVt> {
        Arc::new(LuaVt {
            sender: self.dialog_tx.clone(),
            settings: self.settings.settings.clone(),
        })
    }

    pub async fn collect_process_info() -> Task<Message> {
        match process_info::ProcessInfo::open().await {
            Ok(summary) => Task::done(Message::ProcessInfo(summary)),
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
            WindowType::LuaApi => widget::container(
                widget::container(widget::themer(
                    ::iced_core::Theme::Dark,
                    self.docs_viewer.view().map(Message::LuaDocs),
                ))
                .style(widget::container::dark),
            )
            .padding(5)
            .into(),
            WindowType::Dialog(dialog) => dialog
                .view()
                .map(move |msg| Message::Dialog(id, OrRequest::Message(msg))),
            WindowType::Settings => widget::container(self.settings.view().map(Message::Settings))
                .padding(5)
                .into(),
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
                self.view
                    .view(&self.games, &self.info, &self.process_list, &self.batch),
            )
            .push(vertical_space().height(3))
            .push(horizontal_rule(2))
            .push(vertical_space().height(3))
            .push(
                Row::new()
                    .align_y(Center)
                    .push(text(&self.status).width(Fill))
                    .push(text("Displayed / All").style(widget::text::secondary))
                    .push(horizontal_space().width(5))
                    .push(value(self.games.displayed_count()))
                    .push(text(" / "))
                    .push(value(self.games.all_count()))
                    .push(horizontal_space().width(7))
                    .push(text("Network").style(widget::text::secondary))
                    .push(horizontal_space().width(5))
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
                    ),
            )
            .pipe(Element::from)
    }
}
