use ::core::convert::identity;
use ::std::{path::PathBuf, sync::Arc};

use ::iced_core::{Size, window};
use ::iced_runtime::Task;
use ::image::DynamicImage;
use ::rustix::process::{Pid, RawPid};
use ::spel_katalog_common::{IntoOrRequest, OrRequest};
use ::spel_katalog_formats::NativeGame;
use ::spel_katalog_games::SelDir;
use ::spel_katalog_run::run_umu::RunMode;
use ::spel_katalog_settings::{
    ConfigDir, FilterMode, Load, LutrisDb, Network, Settings, Show, Variants,
};
use ::tap::Pipe;
use ::uuid::Uuid;

use crate::{App, Message, QuickMessage, Safety, app::WindowType};

#[derive(Default)]
#[non_exhaustive]
struct WindowToggleSettings<'a> {
    keep_if_last: bool,
    window_settings: Option<&'a dyn Fn() -> window::Settings>,
}

impl App {
    fn find_windows(
        &self,
        mut condition: impl FnMut(&WindowType) -> bool,
    ) -> impl Iterator<Item = window::Id> {
        self.windows
            .iter()
            .filter_map(move |(k, v)| condition(v).then_some(*k))
    }

    fn toggle_window(
        &self,
        condition: impl FnMut(&WindowType) -> bool,
        factory: impl 'static + Send + FnMut() -> WindowType,
        settings: WindowToggleSettings,
    ) -> Task<Message> {
        let mut windows = self.find_windows(condition).peekable();
        if windows.peek().is_some() {
            if !settings.keep_if_last || self.windows.len() > 1 {
                Task::batch(
                    windows
                        .map(::iced_runtime::window::close)
                        .collect::<Box<[_]>>(),
                )
            } else {
                ::log::info!("refusing to close this window type with no other windows shown");
                Task::none()
            }
        } else {
            let (_, task) = ::iced_runtime::window::open(
                settings
                    .window_settings
                    .map_or_else(Default::default, |factory| factory()),
            );
            let mut factory = factory;
            task.map(move |id| Message::OpenWindow(id, factory()))
        }
    }

    fn convert_all(&self) -> impl 'static + Future<Output = Vec<(Uuid, NativeGame)>> {
        let game_db = self.games_db.clone();
        let futures = self
            .games
            .all()
            .iter()
            .filter_map(|game| self.game_as_native(game.id()))
            .collect::<Vec<_>>();

        async move {
            let mut games = Vec::new();
            for future in futures {
                let Some((game, thumb)) = future.await else {
                    continue;
                };
                games.extend(Self::convert_game(game_db.clone(), game, thumb).await);
            }
            ::log::info!("all games converted");
            games
        }
    }

    async fn convert_game(
        game_db: ::spel_katalog_native::Pool,
        game: NativeGame,
        thumb: Option<DynamicImage>,
    ) -> Option<(Uuid, NativeGame)> {
        ::smol::unblock(move || {
            let name = &game.name;
            let uuid = Uuid::now_v7();
            game_db
                .insert_game(uuid)
                .maybe_thumb(thumb.as_ref())
                .insert(&game)
                .map_err(|err| ::log::error!("failed to insert {name:?} into database\n{err}"))
                .ok()?;

            Some((uuid, game))
        })
        .await
    }

    async fn prefill_installer(
        settings: Arc<Settings>,
        game_dir: PathBuf,
        hidden: Option<bool>,
        thumbnail: Option<PathBuf>,
        move_game: Option<bool>,
    ) -> Option<Task<Message>> {
        let (parent, choice) = ::spel_katalog_installer::Installer::open_path(game_dir).await?;
        let (installer, installer_task) = ::spel_katalog_installer::Installer::new(
            &settings, parent, choice, hidden, thumbnail, move_game,
        );

        let (id, open_task) = ::iced_runtime::window::open(Default::default());

        let add_task = Task::done(Message::OpenWindow(
            id,
            WindowType::Installer(Box::new(installer)),
        ));

        Some(open_task.discard().chain(add_task.chain(
            installer_task.map(move |message| Message::Installer(id, OrRequest::Message(message))),
        )))
    }

    fn open_installer(&mut self, hidden: Option<bool>) -> Task<Message> {
        let settings = self.settings.clone();
        Task::<Option<_>>::future(async move {
            let source = settings
                .get::<::spel_katalog_settings::InstallSource>()
                .to_path_buf();

            let (parent, choice) = ::spel_katalog_installer::Installer::open(source).await?;
            let (installer, installer_task) = ::spel_katalog_installer::Installer::new(
                &settings, parent, choice, hidden, None, None,
            );

            let (id, open_task) = ::iced_runtime::window::open(Default::default());

            let add_task = Task::done(Message::OpenWindow(
                id,
                WindowType::Installer(Box::new(installer)),
            ));

            Some(
                open_task.discard().chain(
                    add_task.chain(
                        installer_task.map(move |message| {
                            Message::Installer(id, OrRequest::Message(message))
                        }),
                    ),
                ),
            )
        })
        .and_then(identity)
    }

    fn quick_update(&mut self, msg: QuickMessage) -> Task<Message> {
        match msg {
            QuickMessage::Debug => {
                ::log::info!("debug action activated");
                return self.quick_update(QuickMessage::OpenInstaller);
            }
            QuickMessage::OpenInstaller => {
                return self.open_installer(None);
            }
            QuickMessage::CopyFilter => {
                return ::iced_runtime::clipboard::write(self.filter.clone());
            }
            QuickMessage::PasteFilter => {
                return ::iced_runtime::clipboard::read()
                    .and_then(|filter| Task::done(Message::Filter(filter)));
            }
            QuickMessage::OpenDatabase => {
                let path = self.settings.get::<ConfigDir>().as_path().join("games.db");
                return Task::<Option<_>>::future(::smol::unblock(move || {
                    if let Err(err) = ::open::that_detached(&path) {
                        ::log::error!("failed to open {path:?}\n{err}");
                    }
                    None
                }))
                .and_then(Task::done);
            }
            QuickMessage::ReloadGames => {
                ::log::info!("reloading games");
                self.games.clear();
                let load_lutris = || {
                    self.settings
                        .get::<LutrisDb>()
                        .to_path_buf()
                        .pipe(move |db_path| spel_katalog_games::Message::LoadDb { db_path })
                        .pipe(OrRequest::Message)
                        .pipe(Message::Games)
                        .pipe(Task::done)
                };

                let load_native = || {
                    let games_db = self.games_db.clone();
                    Task::future(::smol::unblock(move || {
                        let mut games = Vec::new();
                        games_db.gather(&mut |uuid, game| {
                            games.push((uuid, game));
                        });
                        Message::Games(OrRequest::Message(
                            ::spel_katalog_games::Message::AddNativeGames { games },
                        ))
                    }))
                };

                let load_db = match self.settings.get::<Load>() {
                    Load::Lutris => load_lutris(),
                    Load::Native => load_native(),
                    Load::Both => load_native().chain(load_lutris()),
                    Load::None => Task::none(),
                };

                return load_db;
            }
            QuickMessage::ConvertAll => {
                let future = self.convert_all();
                return Task::future(async move {
                    ::spel_katalog_games::Message::AddNativeGames {
                        games: future.await,
                    }
                    .into_message()
                    .pipe(Message::Games)
                });
            }
            QuickMessage::CloseAll => {
                self.view.hide_info();
                self.games.select(SelDir::None);
                self.filter = String::new();
                self.sort_games();
            }
            QuickMessage::ClosePane => {
                if self.view.info_shown() {
                    self.view.hide_info();
                    self.set_status("closed info pane");
                } else if self.games.selected().is_some() {
                    self.games.select(SelDir::None);
                } else if !self.filter.is_empty() {
                    self.filter = String::new();
                    self.sort_games();
                }
            }
            QuickMessage::OpenProcessInfo => {
                self.view.displayed = crate::view::Displayed::Processes;
                self.view.show_info();
            }
            QuickMessage::OpenGameInfo => {
                self.view.displayed = crate::view::Displayed::GameInfo;
                self.view.show_info();
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
                if self.view.displayed.is_processes()
                    && let Some(guard) = self.process_view_semaphore.try_acquire_arc()
                {
                    return Task::future(async move {
                        let value = Self::collect_process_info().await;
                        drop(guard);
                        value
                    })
                    .then(|msg| msg.map_or_else(Task::none, Task::done));
                }
            }
            QuickMessage::Next => return ::iced::widget::operation::focus_next(),
            QuickMessage::Prev => return ::iced::widget::operation::focus_previous(),
            QuickMessage::RunSelected => {
                if let Some(id) = self.games.selected() {
                    return self.run_game(id, Safety::Sandbox, false);
                }
            }
            QuickMessage::ToggleSettings => {
                return self.toggle_window(
                    |t| t.is_settings(),
                    || WindowType::Settings,
                    WindowToggleSettings {
                        window_settings: Some(&|| window::Settings {
                            size: Size {
                                width: 350.0,
                                height: 700.0,
                            },
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                );
            }
            QuickMessage::ToggleProcessInfo => {
                self.view
                    .toggle_displayed(crate::view::Displayed::Processes);
            }
            QuickMessage::ToggleMain => {
                return self.toggle_window(
                    |t| t.is_main(),
                    || WindowType::Main,
                    WindowToggleSettings {
                        keep_if_last: true,
                        ..Default::default()
                    },
                );
            }
            QuickMessage::ToggleGameInfo => {
                self.view.toggle_displayed(crate::view::Displayed::GameInfo);
            }
        }
        Task::none()
    }

    fn games_request(&mut self, request: ::spel_katalog_games::Request) -> Task<Message> {
        match request {
            ::spel_katalog_games::Request::ShowGame { id } => {
                let Self {
                    info,
                    games,
                    sender,
                    settings,
                    view,
                    ..
                } = self;
                if view.info_shown() && view.displayed.is_game_info() && info.id() == Some(id) {
                    view.hide_info();
                } else if let Some(game) = games.by_id(id) {
                    ::log::info!("showing info for {id}");
                    return info
                        .set_game(sender, settings, game, &self.games_db)
                        .map(OrRequest::Message)
                        .map(Message::Info)
                        .then(|message| {
                            Task::done(message).chain(Task::done(Message::ShowInfo(
                                crate::view::Displayed::GameInfo,
                            )))
                        });
                } else {
                    info.clear();
                }
            }
            ::spel_katalog_games::Request::Run { id, sandbox } => {
                return self.run_game(
                    id,
                    if sandbox {
                        Safety::Sandbox
                    } else {
                        Safety::None
                    },
                    false,
                );
            }
            ::spel_katalog_games::Request::CloseInfo => {
                if self.view.displayed.is_game_info() {
                    self.view.hide_info();
                }
                self.games.select(SelDir::None);
            }
            ::spel_katalog_games::Request::Convert(game_id) => {
                if let Some(future) = self.game_as_native(game_id) {
                    let game_db = self.games_db.clone();
                    return Task::<Option<_>>::future(async move {
                        let (game, thumb) = future.await?;
                        let (uuid, config) = Self::convert_game(game_db, game, thumb).await?;

                        ::spel_katalog_games::Message::AddNativeGame {
                            uuid,
                            config: Box::new(config),
                        }
                        .into_message()
                        .pipe(Message::Games)
                        .pipe(Some)
                    })
                    .and_then(Task::done);
                }
            }
            ::spel_katalog_games::Request::InstallGame => {
                return self.open_installer(None);
            }
        }
        Task::none()
    }

    fn info_request(&mut self, request: ::spel_katalog_info::Request) -> Task<Message> {
        match request {
            ::spel_katalog_info::Request::NativeInfo(request) => match request {
                ::spel_katalog_info::NativeRequest::UndisplayThumbnail { id } => {
                    if let Some(game) = self.games.by_id_mut(id) {
                        game.thumb = None;
                    }
                    Task::none()
                }
                ::spel_katalog_info::NativeRequest::DisplayThumbnail { id, img } => {
                    if let Some(game) = self.games.by_id_mut(id) {
                        let ::spel_katalog_formats::Image {
                            width,
                            height,
                            bytes,
                        } = img;
                        let img = ::iced_core::image::Handle::from_rgba(width, height, bytes);
                        game.thumb = Some(img);
                    }
                    Task::none()
                }
                ::spel_katalog_info::NativeRequest::RunGame(game) => {
                    self.run_native_game(*game, RunMode::Exe)
                }
                ::spel_katalog_info::NativeRequest::RunShell(game) => {
                    self.run_native_game(*game, RunMode::Shell)
                }
                ::spel_katalog_info::NativeRequest::RunInit(game) => {
                    self.run_native_game(*game, RunMode::Init)
                }
            },
            ::spel_katalog_info::Request::RemoveImage { slug } => self
                .games
                .update(
                    ::spel_katalog_games::Message::RemoveImage { slug },
                    &self.sender,
                    &self.settings,
                    &self.filter,
                    &self.games_db,
                )
                .map(Message::Games),
            ::spel_katalog_info::Request::SetImage { slug, image } => self
                .games
                .update(
                    ::spel_katalog_games::Message::SetImage {
                        slug,
                        image,
                        add_to_cache: true,
                    },
                    &self.sender,
                    &self.settings,
                    &self.filter,
                    &self.games_db,
                )
                .map(Message::Games),
            ::spel_katalog_info::Request::RunGame { id, sandbox } => {
                self.run_game(id, Safety::from(sandbox), false)
            }
            ::spel_katalog_info::Request::OpenShell { id } => {
                self.run_game(id, Safety::SandboxShell, false)
            }
            ::spel_katalog_info::Request::RunLutrisInSandbox { id } => {
                self.run_game(id, Safety::Sandbox, true)
            }
        }
    }

    fn should_re_sort(msg: &::spel_katalog_settings::Message) -> bool {
        use ::spel_katalog_settings::Delta;
        let ::spel_katalog_settings::Message::Delta(delta) = msg else {
            return false;
        };
        matches!(
            &delta,
            Delta::FilterMode(..) | Delta::Show(..) | Delta::SortBy(..) | Delta::SortDir(..)
        )
    }

    async fn install_game_(
        game_db: ::spel_katalog_native::Pool,
        config: Box<NativeGame>,
        thumbnail: Option<::spel_katalog_formats::Image>,
        move_dir: Option<(PathBuf, PathBuf)>,
    ) -> Option<(Uuid, Box<NativeGame>)> {
        let uuid = Uuid::now_v7();
        game_db
            .insert_game(uuid)
            .insert(&config)
            .map_err(|err| {
                ::log::error!(
                    "could not insert game {:?} into database\n{err}",
                    config.name
                )
            })
            .ok()?;

        let thumbnail = thumbnail.and_then(::spel_katalog_formats::Image::into_image);
        if let Some(thumbnail) = &thumbnail
            && let Err(err) = game_db.insert_thumb(uuid).insert(thumbnail)
        {
            ::log::warn!("could not insert thumbnail for {uuid}\n{err}");
        }

        if let Some((src, dest)) = move_dir
            && let Err(err) = ::smol::fs::rename(&src, &dest).await
        {
            ::log::error!("could not rename {src:?} to {dest:?}\n{err}");
            if let Err(err) = game_db.remove_game(uuid) {
                ::log::error!(
                    "cleanup of inserted game failed, database might still contain config\n{err}"
                );
            }
            return None;
        }

        Some((uuid, config))
    }

    pub fn install_game(
        &mut self,
        id: window::Id,
        config: Box<NativeGame>,
        thumbnail: Option<::spel_katalog_formats::Image>,
        move_dir: Option<(PathBuf, PathBuf)>,
    ) -> Task<Message> {
        let game_db = self.games_db.clone();
        Self::install_game_(game_db, config, thumbnail, move_dir)
            .pipe(Task::future)
            .and_then(move |(uuid, config)| {
                ::spel_katalog_games::Message::AddNativeGame { uuid, config }
                    .into_message()
                    .pipe(Message::Games)
                    .pipe(Task::done)
                    .chain(::iced_runtime::window::close(id))
            })
    }

    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Quick(quick) => return self.quick_update(quick),
            Message::Status(status) => {
                self.set_status(status);
                return Task::none();
            }
            Message::Filter(filter) => {
                self.filter = filter;
                self.games.sort(&self.settings, &self.filter);
            }
            Message::Settings(message) => {
                return if Self::should_re_sort(&message) {
                    let task = self.settings.update(message, &self.sender);
                    self.sort_games();
                    task
                } else {
                    self.settings.update(message, &self.sender)
                }
                .map(Message::Settings);
            }
            Message::View(message) => return self.view.update(message),
            Message::Terminal(message) => return self.terminal.update(message).map(From::from),
            Message::Games(message) => match message {
                OrRequest::Message(message) => {
                    return self
                        .games
                        .update(
                            message,
                            &self.sender,
                            &self.settings,
                            &self.filter,
                            &self.games_db,
                        )
                        .map(Message::Games);
                }
                OrRequest::Request(request) => return self.games_request(request),
            },

            Message::Info(message) => match message {
                OrRequest::Message(message) => {
                    return self
                        .info
                        .update(
                            message,
                            &self.sender,
                            &self.settings,
                            &|id| self.games.by_id(id).map(|g| &g.game),
                            &self.games_db,
                        )
                        .map(Message::Info);
                }
                OrRequest::Request(request) => return self.info_request(request),
            },

            Message::ProcessInfo(process_infos) => {
                self.process_list = process_infos;
            }
            Message::Kill { pid, terminate } => {
                let Ok(pid) = RawPid::try_from(pid) else {
                    return Task::none();
                };
                let Some(pid) = Pid::from_raw(pid) else {
                    return Task::none();
                };

                return Task::future(async move {
                    let result = ::smol::unblock(move || {
                        ::rustix::process::kill_process(
                            pid,
                            if terminate {
                                ::rustix::process::Signal::TERM
                            } else {
                                ::rustix::process::Signal::KILL
                            },
                        )
                    })
                    .await;
                    match result {
                        Ok(_) => ::log::info!(
                            "sent TERM to process {pid}",
                            pid = pid.as_raw_nonzero().get()
                        ),
                        Err(err) => ::log::error!(
                            "could not kill process {pid}\n{err}",
                            pid = pid.as_raw_nonzero().get()
                        ),
                    };
                })
                .then(|_| Task::none());
            }
            Message::OpenWindow(id, window_type) => {
                self.windows.insert(id, window_type);
            }
            Message::CloseWindow(id) => {
                let closed = self.windows.remove(&id);

                if self.windows.is_empty() || matches!(closed, Some(WindowType::Term)) {
                    self.sink_builder = ::spel_katalog_sink::SinkBuilder::Inherit;
                    return ::iced_runtime::exit();
                }
            }
            Message::Installer(id, msg) => {
                if let Some(WindowType::Installer(installer)) = self.windows.get_mut(&id) {
                    return match msg {
                        OrRequest::Message(msg) => installer
                            .update(msg, &self.settings)
                            .map(move |msg| Message::Installer(id, msg)),
                        OrRequest::Request(req) => match req {
                            ::spel_katalog_installer::Request::Close => {
                                ::iced_runtime::window::close(id)
                            }
                            ::spel_katalog_installer::Request::InstallGame {
                                config,
                                thumbnail,
                                move_dir,
                            } => self.install_game(id, config, thumbnail, move_dir),
                        },
                    };
                }
            }
            Message::Ipc(message) => match message {
                ::spel_katalog_ipc::Message::InstallGame {
                    source,
                    hidden,
                    thumbnail,
                    move_game,
                } => {
                    return Task::future(Self::prefill_installer(
                        self.settings.snapshot(),
                        source,
                        Some(hidden),
                        thumbnail,
                        Some(move_game),
                    ))
                    .and_then(identity);
                }
            },
            Message::ShowInfo(displayed) => {
                self.view.displayed = displayed;
                self.view.show_info();
            }
            Message::RunGameNative(game) => {
                return self.run_native_game(*game, RunMode::Exe);
            }
            Message::RunShellNative(game) => {
                return self.run_native_game(*game, RunMode::Shell);
            }
        }
        Task::none()
    }
}
