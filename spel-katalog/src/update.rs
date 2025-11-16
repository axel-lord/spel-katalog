use ::std::{convert::identity, path::Path};

use ::iced_core::{Size, window};
use ::iced_runtime::Task;
use ::iced_widget as widget;
use ::rustc_hash::FxHashMap;
use ::rustix::process::{Pid, RawPid};
use ::spel_katalog_batch::BatchInfo;
use ::spel_katalog_common::OrRequest;
use ::spel_katalog_formats::AdditionalConfig;
use ::spel_katalog_games::SelDir;
use ::spel_katalog_settings::{ConfigDir, FilterMode, Network, Show, Variants, YmlDir};
use ::tap::Pipe;

use crate::{App, Message, QuickMessage, Safety, app::WindowType};

pub fn gather<'a>(
    yml_dir: &str,
    config_dir: &str,
    games: impl IntoIterator<Item = &'a ::spel_katalog_formats::Game>,
) -> Vec<BatchInfo> {
    games
        .into_iter()
        .map(|game| BatchInfo {
            id: game.id,
            slug: game.slug.clone(),
            name: game.name.clone(),
            runner: game.runner.to_string(),
            config: format!("{yml_dir}/{}.yml", game.configpath),
            hidden: game.hidden,
            attrs: 'attrs: {
                let path = format!("{config_dir}/games/{}.toml", game.id);
                let path = Path::new(&path);

                if !path.exists() {
                    break 'attrs FxHashMap::default();
                }

                ::std::fs::read_to_string(path)
                    .map_err(|err| ::log::error!("could not read additional path {path:?}\n{err}"))
                    .ok()
                    .and_then(|content| {
                        let additional = ::toml::from_str::<AdditionalConfig>(&content)
                            .map_err(|err| ::log::error!("could not parse toml of {path:?}\n{err}"))
                            .ok()?;
                        Some(additional.attrs)
                    })
                    .unwrap_or_default()
            },
        })
        .collect()
}

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

    fn quick_update(&mut self, msg: QuickMessage) -> Task<Message> {
        match msg {
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
                if self.view.displayed.is_processes() {
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
            QuickMessage::ToggleLuaApi => {
                return self.toggle_window(
                    |t| t.is_lua_api(),
                    || WindowType::LuaApi,
                    WindowToggleSettings::default(),
                );
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
                match (self.view.info_shown(), self.view.displayed) {
                    (true, crate::view::Displayed::Processes) => {
                        self.view.hide_info();
                    }
                    _ => {
                        self.view.show_info();
                        self.view.displayed = crate::view::Displayed::Processes;
                    }
                }
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
                    return info
                        .set_game(sender, settings, game)
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
                        Safety::Firejail
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
        }
        Task::none()
    }

    fn info_request(&mut self, request: ::spel_katalog_info::Request) -> Task<Message> {
        match request {
            ::spel_katalog_info::Request::RemoveImage { slug } => self
                .games
                .update(
                    ::spel_katalog_games::Message::RemoveImage { slug },
                    &self.sender,
                    &self.settings,
                    &self.filter,
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
                )
                .map(Message::Games),
            ::spel_katalog_info::Request::RunGame { id, sandbox } => {
                self.run_game(id, Safety::from(sandbox), false)
            }
            ::spel_katalog_info::Request::RunLutrisInSandbox { id } => {
                self.run_game(id, Safety::Firejail, true)
            }
        }
    }

    fn batch_request(&mut self, request: ::spel_katalog_batch::Request) -> Task<Message> {
        match request {
            ::spel_katalog_batch::Request::ShowProcesses => {
                return Task::done(Message::Quick(QuickMessage::OpenProcessInfo));
            }
            ::spel_katalog_batch::Request::HideBatch => self.show_batch = false,
            ::spel_katalog_batch::Request::GatherBatchInfo(scope) => {
                let yml_dir = self.settings.get::<YmlDir>();
                let yml_dir = yml_dir.as_str();
                let config_dir = self.settings.get::<ConfigDir>().as_str();
                return match scope {
                    ::spel_katalog_batch::Scope::All => gather(
                        yml_dir,
                        config_dir,
                        self.games.all().iter().map(|game| &game.game),
                    ),
                    ::spel_katalog_batch::Scope::Shown => gather(
                        yml_dir,
                        config_dir,
                        self.games.displayed().map(|game| &game.game),
                    ),
                    ::spel_katalog_batch::Scope::Batch => gather(
                        yml_dir,
                        config_dir,
                        self.games.batch_selected().map(|game| &game.game),
                    ),
                }
                .pipe(::spel_katalog_batch::Message::RunBatch)
                .pipe(OrRequest::Message)
                .pipe(Message::Batch)
                .pipe(Task::done);
            }
            ::spel_katalog_batch::Request::ReloadCache => {
                return self.games.find_cached(&self.settings).map(Message::Games);
            }
        }
        Task::none()
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
                        .update(message, &self.sender, &self.settings, &self.filter)
                        .map(Message::Games);
                }
                OrRequest::Request(request) => return self.games_request(request),
            },

            Message::Info(message) => match message {
                OrRequest::Message(message) => {
                    return self
                        .info
                        .update(message, &self.sender, &self.settings, &|id| {
                            self.games.by_id(id).map(|g| &g.game)
                        })
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
            Message::Batch(or_request) => match or_request {
                OrRequest::Message(msg) => {
                    return self
                        .batch
                        .update(
                            msg,
                            &self.sender,
                            &self.settings,
                            &self.sink_builder,
                            self.lua_vt(),
                        )
                        .map(From::from);
                }
                OrRequest::Request(request) => return self.batch_request(request),
            },
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
            Message::Dialog(id, msg) => {
                let msg = match msg {
                    OrRequest::Message(msg) => msg,
                    OrRequest::Request(request) => match request {
                        crate::dialog::Request::Close => return ::iced_runtime::window::close(id),
                    },
                };
                if let Some(WindowType::Dialog(dialog)) = self.windows.get_mut(&id) {
                    return dialog
                        .update(msg)
                        .map(move |request| Message::Dialog(id, OrRequest::Request(request)));
                }
            }
            Message::BuildDialog(dialog) => {
                let (_, task) = ::iced_runtime::window::open(window::Settings {
                    size: Size {
                        width: 500.0,
                        height: 250.0,
                    },
                    position: window::Position::Centered,
                    ..Default::default()
                });
                return task.map(move |id| {
                    Message::OpenWindow(id, WindowType::Dialog(dialog.clone().build()))
                });
            }
            Message::LuaDocs(msg) => {
                return self.docs_viewer.update(msg).map(Message::LuaDocs);
            }
            Message::ShowInfo(displayed) => {
                self.view.displayed = displayed;
                self.view.show_info();
            }
        }
        Task::none()
    }
}
