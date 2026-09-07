//! Application state implementation.

use ::core::convert::identity;
use ::std::io::PipeReader;

use ::color_eyre::{Section, eyre::eyre};
use ::derive_more::IsVariant;
use ::iced_core::{Alignment::Center, Font, Length::Fill, font, window};
use ::iced_runtime::Task;
use ::iced_widget::{self as widget, Column, Container, Row, text, text_input, toggler, value};
use ::rustc_hash::FxHashMap;
use ::spel_katalog_cli::Run;
use ::spel_katalog_common::{OrRequest, StatusSender, w};
use ::spel_katalog_enthread::enthread;
use ::spel_katalog_formats::TagStorage;
use ::spel_katalog_installer::Installer;
use ::spel_katalog_list_menu::ListMenu;
use ::spel_katalog_process_view::ProcessView;
use ::spel_katalog_settings::{FilterMode, Network, Theme, UseWayland};
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};
use ::spel_katalog_tag_filter::TagFilterDialog;
use ::tap::Pipe;

use crate::{
    Element, ExitReceiver, Message, QuickMessage, get_settings,
    pane_view::{self, PaneView},
    view,
};

/// Specific kind of window.
#[derive(Debug, IsVariant, Clone)]
pub enum WindowType {
    /// Window is the main window.
    Main,
    /// Show a settings window.
    Settings,
    /// Show an installer window.
    Installer(Box<Installer>),
    /// Show a pane view window.
    PaneView(crate::pane_view::State),
}

/// Currently viewed popup.
#[derive(Debug, IsVariant)]
pub enum Popup {
    /// Welcome popup.
    Welcome,
    /// Tag filter popup.
    TagFilter,
}

/// Application state.
#[derive(Debug)]
pub(crate) struct App {
    /// Settings state.
    pub settings: ::spel_katalog_settings_view::State,
    /// Games view state.
    pub games: ::spel_katalog_games::State,
    /// Current status message.
    pub status: String,
    /// Current filter.
    pub filter: String,
    /// View state.
    pub view: view::State,
    /// Info panel state.
    pub info: ::spel_katalog_info::State,
    /// Sender for status messages.
    pub sender: StatusSender,
    /// Sink builder for output.
    pub sink_builder: SinkBuilder,
    /// Current windows.
    pub windows: FxHashMap<window::Id, WindowType>,
    /// Terminal pane state.
    pub terminal: ::spel_katalog_terminal::Terminal,
    /// Database connection.
    pub games_db: ::spel_katalog_native::Pool,
    /// Tags in use.
    pub tags: TagStorage,
    /// Tag filter applied.
    pub tag_filter: TagFilterDialog,
    /// Current popup (if any).
    pub popup: Option<Popup>,
    /// Process view panel state.
    pub process_view: ProcessView,
}

/// Initial state created by new.
#[derive(Debug)]
struct Initial {
    /// Application state to use.
    app: App,
    /// Status receiver.
    status_rx: ::flume::Receiver<String>,
    /// Terminal receiver (if any).
    terminal_rx: Option<::flume::Receiver<(PipeReader, SinkIdentity)>>,
    /// Should settings be shown.
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
    /// Create new initial state.
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
        let settings = ::spel_katalog_settings_view::State { settings, config };
        let games = ::spel_katalog_games::State::default();
        let info = ::spel_katalog_info::State::default();
        let sender = status_tx.into();
        let windows = FxHashMap::default();
        let terminal = ::spel_katalog_terminal::Terminal::default().with_limit(256);
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
        let tags = TagStorage::default();
        let popup = None;
        let tag_filter = TagFilterDialog::new();
        let process_view = ProcessView::default();

        let app = App {
            filter,
            games,
            info,
            sender,
            settings,
            sink_builder,
            status,
            terminal,
            view,
            windows,
            games_db,
            tags,
            popup,
            tag_filter,
            process_view,
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
    /// Create new application state.
    fn new(
        Flags {
            initial:
                Initial {
                    mut app,
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

        let open_term = terminal_rx.map_or_default(|terminal_rx| {
            let (id, open_term) = ::iced_runtime::window::open(Default::default());
            app.open_window(
                id,
                WindowType::PaneView(
                    pane_view::State::builder(pane_view::Pane::Log)
                        .horizontal(pane_view::Pane::Terminal)
                        .build(),
                ),
            );
            Task::batch([
                open_term.discard(),
                ::spel_katalog_terminal::Message::sink_receiver(terminal_rx).map(Message::Terminal),
            ])
        });

        let show_settings = if show_settings {
            QuickMessage::ToggleSettings
                .pipe(Message::Quick)
                .pipe(Task::done)
        } else {
            Task::none()
        };

        let load_db = QuickMessage::ReloadGames
            .pipe(Message::Quick)
            .pipe(Task::done);

        let (tx, rx) = ::flume::unbounded::<Message>();
        let xdg = app.settings.xdg().clone();

        let thread = enthread("spel-katalog-ipc-listener", async move || {
            let listener =
                match ::spel_katalog_ipc::IpcListener::create("spel-katalog-ipc", &xdg).await {
                    Ok(listener) => listener,
                    Err(err) => {
                        ::log::error!("could not create ipc listener\n{err}");
                        return;
                    }
                };

            let Err(err) = listener
                .resolve(async |resolver| {
                    resolver
                        .post(async |resolver| {
                            resolver
                                .resource(async |message| {
                                    tx.send_async(Message::Ipc(message)).await?;
                                    Ok(())
                                })
                                .await
                        })
                        .await
                })
                .await;

            ::log::error!("ipc error\n{err}");
        });

        let listen_ipc = if let Err(err) = thread {
            ::log::error!("could not create ipc thread\n{err}");
            Task::none()
        } else {
            Task::stream(rx.into_stream())
        };

        let xdg = app.settings.xdg().clone();
        let receive_ids = Task::future(
            async move { ::spel_katalog_api::get_running_games(&xdg).await },
        )
        .then(|children| match children {
            Ok(::spel_katalog_formats::daemon::response::Children { children }) => children
                .into_iter()
                .map(|pid| Message::ViewProcess { pid })
                .pipe(::smol::stream::iter)
                .pipe(Task::stream),
            Err(err) => {
                ::log::error!("could not get running game process ids\n{err}");
                Task::none()
            }
        });

        let batch = Task::batch([
            receive_status,
            load_db,
            main,
            exit_recv,
            open_term,
            show_settings,
            listen_ipc,
            receive_ids,
        ]);

        (app, batch)
    }

    /// Run application.
    pub fn run(
        run: Run,
        sink_builder: SinkBuilder,
        _exit_recv: Option<ExitReceiver>,
    ) -> ::color_eyre::Result<()> {
        ::iced_winit::run(Program { run, sink_builder }).map_err(|err| eyre!(err))
    }

    /// Sort games.
    pub fn sort_games(&mut self) {
        self.games.sort(&self.settings, &self.filter);
    }

    /// Set status message and log it.
    pub fn set_status(&mut self, status: impl Into<String>) {
        let status = status.into();
        ::log::info!("status: {status}");
        self.status = status;
    }

    /// View application ui.
    pub fn view(&self, id: window::Id) -> Element<'_, Message> {
        let Some(ty) = self.windows.get(&id) else {
            return widget::container("No Window Type").center(Fill).into();
        };

        match ty {
            WindowType::Main => self.view_main(),
            WindowType::Settings => widget::container(self.settings.view().map(Message::Settings))
                .padding(5)
                .into(),
            WindowType::Installer(installer) => installer
                .view(&self.settings)
                .map(move |msg| Message::Installer(id, msg)),
            WindowType::PaneView(state) => PaneView {
                state,
                settings: &self.settings,
                terminal: &self.terminal,
                process_view: &self.process_view,
                info: &self.info,
                games: &self.games,
            }
            .view(id),
        }
    }

    /// View the main column of content.
    pub fn main_column(&self) -> Column<'_, Message> {
        fn with_global_context(menu: ListMenu<'_, Message>) -> ListMenu<'_, Message> {
            menu.label("Spel Katalog")
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
                            .label("Filter")
                            .separator()
                            .button("Copy", || Message::Quick(QuickMessage::CopyFilter))
                            .button("Paste", || Message::Quick(QuickMessage::PasteFilter))
                            .button("Tags", || Message::Quick(QuickMessage::ShowTagFilter))
                            .separator()
                            .pipe(with_global_context)
                            .into()
                    })
                }),
            )
            .push(widget::space::vertical().height(5))
            .push(
                self.view
                    .view(&self.games, &self.info, &self.settings, &self.process_view),
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
                    .push(text("Wayland").style(widget::text::secondary))
                    .push(widget::space::horizontal().width(5))
                    .push(
                        toggler(self.settings.get::<UseWayland>().is_enabled())
                            .spacing(0)
                            .on_toggle(|wl| {
                                Message::Settings(::spel_katalog_settings_view::Message::Delta(
                                    spel_katalog_settings::Delta::UseWayland(match wl {
                                        true => UseWayland::Enabled,
                                        false => UseWayland::Disabled,
                                    }),
                                ))
                            }),
                    )
                    .push(widget::space::horizontal().width(5))
                    .push(text("Network").style(widget::text::secondary))
                    .push(widget::space::horizontal().width(5))
                    .push(
                        toggler(self.settings.get::<Network>().is_enabled())
                            .spacing(0)
                            .on_toggle(|net| {
                                Message::Settings(::spel_katalog_settings_view::Message::Delta(
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
    }

    /// View welcome popup.
    pub fn view_welcome(&self) -> Container<'_, Message> {
        widget::Column::new()
            .push(widget::text("Welcome to spel-katalog!"))
            .push(widget::text("Välkomen till spel-katalog!"))
            .align_x(Center)
            .pipe(widget::container)
            .style(widget::container::bordered_box)
            .padding(30)
    }

    /// View tag filter popup.
    pub fn view_tag_filter(&self) -> Container<'_, Message> {
        self.tag_filter
            .view(&self.tags)
            .map(OrRequest::Message)
            .map(Message::TagFilter)
            .pipe(widget::container)
            .style(widget::container::bordered_box)
            .padding(10)
    }

    /// View main window.
    pub fn view_main(&self) -> Element<'_, Message> {
        let main_column = self.main_column();
        if let Some(popup) = &self.popup {
            widget::Stack::new()
                .height(Fill)
                .width(Fill)
                .push(main_column)
                .push(
                    widget::space()
                        .pipe(widget::center)
                        .style(|t: &::iced_core::Theme| {
                            widget::container::background(
                                t.extended_palette()
                                    .background
                                    .weakest
                                    .color
                                    .scale_alpha(0.7),
                            )
                        })
                        .pipe(widget::mouse_area)
                        .on_release(Message::Quick(QuickMessage::EscapeOne))
                        .pipe(widget::opaque),
                )
                .push(
                    match popup {
                        Popup::Welcome => self.view_welcome(),
                        Popup::TagFilter => self.view_tag_filter(),
                    }
                    .pipe(widget::opaque)
                    .pipe(widget::center),
                )
                .into()
        } else {
            main_column.into()
        }
    }
}

#[derive(Debug)]
struct Program {
    run: Run,
    sink_builder: SinkBuilder,
}

impl ::iced_winit::program::Program for Program {
    type State = App;

    type Message = Message;

    type Theme = ::iced_core::Theme;

    type Renderer = ::iced_widget::Renderer;

    type Executor = ::iced_futures::backend::native::smol::Executor;

    fn name() -> &'static str {
        "spel-katalog"
    }

    fn title(&self, _state: &Self::State, _window: window::Id) -> String {
        "Spel-Katalog".to_owned()
    }

    fn subscription(&self, state: &Self::State) -> ::iced_futures::Subscription<Self::Message> {
        state.subscription()
    }

    fn settings(&self) -> ::iced_core::Settings {
        ::iced_core::Settings {
            default_font: Font {
                weight: font::Weight::Medium,
                ..Font::default()
            },
            ..::iced_core::Settings::default()
        }
    }

    fn theme(&self, state: &Self::State, _window: window::Id) -> Option<Self::Theme> {
        Some(::spel_katalog_settings_view::conv_theme(
            *state.settings.get::<Theme>(),
        ))
    }

    fn window(&self) -> Option<window::Settings> {
        None
    }

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        App::new(Flags {
            initial: Initial::new(self.run.clone(), self.sink_builder.clone())
                .expect("should be able to create initial state"),
            exit_recv: None,
        })
    }

    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
        state.update(message)
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
    ) -> iced_core::Element<'a, Self::Message, Self::Theme, Self::Renderer> {
        state.view(window)
    }
}
