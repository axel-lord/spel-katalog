use ::std::{convert::identity, path::Path};

use ::iced::{
    Size, Task,
    widget::{self},
    window,
};
use ::rustc_hash::FxHashMap;
use ::rustix::process::{Pid, RawPid};
use ::spel_katalog_batch::BatchInfo;
use ::spel_katalog_common::OrRequest;
use ::spel_katalog_games::SelDir;
use ::spel_katalog_info::formats::Additional;
use ::spel_katalog_settings::{
    CoverartDir, ExtraConfigDir, FilterMode, Network, Show, Variants, YmlDir,
};
use ::tap::Pipe;

use crate::{App, Message, QuickMessage, Safety, app::WindowType};

impl App {
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
                    ::spel_katalog_info::Request::RemoveImage { slug } => {
                        return self
                            .games
                            .update(
                                ::spel_katalog_games::Message::RemoveImage { slug },
                                &self.sender,
                                &self.settings,
                                &self.filter,
                            )
                            .map(Message::Games);
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
                QuickMessage::OpenLua => {
                    let (_, task) = window::open(window::Settings::default());
                    return task.map(|id| Message::OpenWindow(id, WindowType::LuaApi));
                }
            },
            Message::ProcessInfo(process_infos) => {
                self.process_list = process_infos.filter(|infos| !infos.is_empty())
            }
            Message::Kill { pid, terminate } => {
                let Ok(pid) = RawPid::try_from(pid) else {
                    return Task::none();
                };
                let Some(pid) = Pid::from_raw(pid) else {
                    return Task::none();
                };

                return Task::future(async move {
                    match ::tokio::task::spawn_blocking(move || {
                        ::rustix::process::kill_process(
                            pid,
                            if terminate {
                                ::rustix::process::Signal::TERM
                            } else {
                                ::rustix::process::Signal::KILL
                            },
                        )
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
                    let lua_vt = self.lua_vt();
                    return self
                        .batch
                        .update(
                            msg,
                            &self.sender,
                            &self.settings,
                            &self.sink_builder,
                            &|| lua_vt.clone(),
                        )
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
                            extra_config_dir: &str,
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
                                    attrs: 'attrs: {
                                        let path = format!("{extra_config_dir}/{}.toml", game.id);
                                        let path = Path::new(&path);

                                        if !path.exists() {
                                            break 'attrs FxHashMap::default();
                                        }

                                        ::std::fs::read_to_string(path)
                                            .map_err(|err| {
                                                ::log::error!(
                                                    "could not read additional path {path:?}\n{err}"
                                                )
                                            })
                                            .ok()
                                            .and_then(|content| {
                                                let additional = ::toml::from_str::<Additional>(
                                                    &content,
                                                )
                                                .map_err(|err| {
                                                    ::log::error!(
                                                        "could not parse toml of {path:?}\n{err}"
                                                    )
                                                })
                                                .ok()?;
                                                Some(additional.attrs)
                                            })
                                            .unwrap_or_default()
                                    },
                                })
                                .collect::<Vec<_>>()
                                .pipe(::spel_katalog_batch::Message::RunBatch)
                                .pipe(OrRequest::Message)
                                .pipe(Message::Batch)
                                .pipe(Task::done)
                        }
                        let yml_dir = self.settings.get::<YmlDir>();
                        let yml_dir = yml_dir.as_str();
                        let extra_config_dir = self.settings.get::<ExtraConfigDir>();
                        let extra_config_dir = extra_config_dir.as_str();
                        return match scope {
                            ::spel_katalog_batch::Scope::All => {
                                gather(yml_dir, extra_config_dir, self.games.all())
                            }
                            ::spel_katalog_batch::Scope::Shown => {
                                gather(yml_dir, extra_config_dir, self.games.displayed())
                            }
                            ::spel_katalog_batch::Scope::Batch => {
                                gather(yml_dir, extra_config_dir, self.games.batch_selected())
                            }
                        };
                    }
                    ::spel_katalog_batch::Request::ReloadCache => {
                        return self.games.find_cached(&self.settings).map(Message::Games);
                    }
                },
            },
            Message::OpenWindow(id, window_type) => {
                self.windows.insert(id, window_type);
            }
            Message::CloseWindow(id) => {
                self.windows.remove(&id);

                if self.windows.is_empty() {
                    return ::iced::exit();
                }
            }
            Message::Url(url) => {
                ::log::info!("markdown url clicked {url}");
            }
            Message::DialogRequest(id, request) => match request {
                crate::dialog::Request::Close => return window::close(id),
            },
            Message::DialogMessage(id, msg) => {
                if let Some(WindowType::Dialog(dialog)) = self.windows.get_mut(&id) {
                    return dialog
                        .update(msg)
                        .map(move |request| Message::DialogRequest(id, request));
                }
            }
            Message::Dialog(dialog) => {
                let (_, task) = window::open(window::Settings {
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
        }
        Task::none()
    }
}
