use ::std::{convert::identity, io::PipeReader, sync::Arc};

use ::color_eyre::{Section, eyre::eyre};
use ::derive_more::IsVariant;
use ::iced::Font;
use ::iced_core::{Alignment::Center, Length::Fill, font, window};
use ::iced_runtime::Task;
use ::iced_widget::{self as widget, Row, text, text_input, toggler, value};
use ::rustc_hash::FxHashMap;
use ::spel_katalog_cli::Run;
use ::spel_katalog_common::{StatusSender, w};
use ::spel_katalog_installer::Installer;
use ::spel_katalog_settings::{FilterMode, Network, Theme};
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};
use ::spel_katalog_widget::ListMenu;
use ::tap::Pipe;

use crate::{Element, ExitReceiver, Message, QuickMessage, get_settings, process_info, view};

/// Specific kind of window.
#[derive(Debug, IsVariant, Clone)]
pub enum WindowType {
    /// Window is the main window.
    Main,
    /// Show terminal.
    Term,
    /// Show a settings window.
    Settings,
    /// Show an installer window.
    Installer(Box<Installer>),
}

#[derive(Debug)]
pub(crate) struct App {
    pub settings: ::spel_katalog_settings::State,
    pub games: ::spel_katalog_games::State,
    pub status: String,
    pub filter: String,
    pub view: view::State,
    pub info: ::spel_katalog_info::State,
    pub sender: StatusSender,
    pub process_list: Vec<process_info::ProcessInfo>,
    pub sink_builder: SinkBuilder,
    pub windows: FxHashMap<window::Id, WindowType>,
    pub terminal: ::spel_katalog_terminal::Terminal,
    pub process_view_semaphore: Arc<::smol::lock::Semaphore>,
    pub games_db: ::spel_katalog_native::Pool,
}

/// Initial state created by new.
#[derive(Debug)]
struct Initial {
    app: App,
    status_rx: ::flume::Receiver<String>,
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
        let terminal = ::spel_katalog_terminal::Terminal::default().with_limit(256);
        let process_view_semaphore = Arc::new(::smol::lock::Semaphore::new(1));
        let games_db = ::spel_katalog_native::Pool::new(
            &settings
                .xdg()
                .place_config_file("games.db")
                .map_err(|err| eyre!(err).note("does home exist?"))?,
        )?;

        let (sink_builder, terminal_rx) = if show_terminal {
            let (terminal_tx, terminal_rx) = ::flume::unbounded();
            (SinkBuilder::CreatePipe(terminal_tx), Some(terminal_rx))
        } else {
            (sink_builder, None)
        };

        let app = App {
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
            process_view_semaphore,
            games_db,
        };

        Ok(Self {
            app,
            status_rx,
            terminal_rx,
            show_settings,
        })
    }
}

impl App {
    fn new(
        Flags {
            initial:
                Initial {
                    app,
                    status_rx,
                    terminal_rx,
                    show_settings,
                },
            exit_recv,
        }: Flags,
    ) -> (Self, Task<Message>) {
        let (_, open_main) = ::iced_runtime::window::open(::iced_core::window::Settings::default());
        let main = open_main.map(|id| Message::OpenWindow(id, WindowType::Main));

        let receive_status = Task::stream(status_rx.into_stream()).map(Message::Status);
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

        let load_db = QuickMessage::ReloadGames
            .pipe(Message::Quick)
            .pipe(Task::done);

        let listen_ipc =
            Task::stream(::spel_katalog_ipc::listen(app.settings.xdg())).map(Message::from);

        let batch = Task::batch([
            receive_status,
            load_db,
            main,
            exit_recv,
            window_recv,
            show_settings,
            listen_ipc,
        ]);

        (app, batch)
    }

    pub fn run(
        run: Run,
        sink_builder: SinkBuilder,
        _exit_recv: Option<ExitReceiver>,
    ) -> ::color_eyre::Result<()> {
        ::iced::daemon(
            move || {
                Self::new(Flags {
                    initial: Initial::new(run.clone(), sink_builder.clone())
                        .expect("should be able to create initial state"),
                    exit_recv: None,
                })
            },
            Self::update,
            Self::view,
        )
        .title(|_: &Self, _| "Spel-Katalog".to_owned())
        .subscription(Self::subscription)
        .default_font(Font {
            weight: font::Weight::Medium,
            ..Font::DEFAULT
        })
        .theme(|this: &Self, _: window::Id| {
            Some(::iced_core::Theme::from(*this.settings.get::<Theme>()))
        })
        .run()
        .map_err(|err| ::color_eyre::eyre::eyre!(err))
    }

    pub async fn collect_process_info() -> Option<Message> {
        match process_info::ProcessInfo::open().await {
            Ok(summary) => Some(Message::ProcessInfo(summary)),
            Err(err) => {
                ::log::error!("whilst collecting info\n{err}");
                None
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
            WindowType::Settings => widget::container(self.settings.view().map(Message::Settings))
                .padding(5)
                .into(),
            WindowType::Term => self.terminal.view().map(From::from),
            WindowType::Installer(installer) => installer
                .view(&self.settings)
                .map(move |msg| Message::Installer(id, msg)),
        }
    }

    pub fn view_main(&self) -> Element<'_, Message> {
        fn with_global_context(menu: ListMenu<'_, Message>) -> ListMenu<'_, Message> {
            menu.push(widget::text("Spel Katalog"))
                .separator()
                .button("Install Game", || {
                    Message::Quick(QuickMessage::OpenInstaller)
                })
                .button("Convert All", || Message::Quick(QuickMessage::ConvertAll))
                .button("Open DB", || Message::Quick(QuickMessage::OpenDatabase))
                .button("Reload Games", || Message::Quick(QuickMessage::ReloadGames))
        }
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
                .map(Message::Filter)
                .pipe(|element| {
                    ::iced_aw::ContextMenu::new(element, || {
                        ListMenu::new()
                            .push(widget::text("Filter"))
                            .separator()
                            .button("Copy", || Message::Quick(QuickMessage::CopyFilter))
                            .button("Paste", || Message::Quick(QuickMessage::PasteFilter))
                            .separator()
                            .pipe(with_global_context)
                            .into()
                    })
                }),
            )
            .push(widget::space::vertical().height(5))
            .push(
                self.view
                    .view(&self.games, &self.info, &self.process_list, &self.settings),
            )
            .push(widget::space::vertical().height(3))
            .push(spel_katalog_widget::rule::horizontal())
            .push(widget::space::vertical().height(3))
            .push(
                Row::new()
                    .align_y(Center)
                    .push(text(&self.status).width(Fill))
                    .push(text("Displayed / All").style(widget::text::secondary))
                    .push(widget::space::horizontal().width(5))
                    .push(value(self.games.displayed_count()))
                    .push(text(" / "))
                    .push(value(self.games.all_count()))
                    .push(widget::space::horizontal().width(7))
                    .push(text("Network").style(widget::text::secondary))
                    .push(widget::space::horizontal().width(5))
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
                    .pipe(|statusbar| {
                        ::iced_aw::ContextMenu::new(statusbar, || {
                            ListMenu::new().pipe(with_global_context).into()
                        })
                    }),
            )
            .pipe(Element::from)
    }
}
