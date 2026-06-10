use ::std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use ::color_eyre::eyre::eyre;
use ::iced_runtime::Task;
use ::image::DynamicImage;
use ::spel_katalog_common::status;
use ::spel_katalog_formats::{AdditionalConfig, Game, GameId, NativeGame, lutris_config};
use ::spel_katalog_run::{
    Callback,
    run_umu::{CommonUmuCtx, LutrisCtx, LutrisUmuCtx, NativeUmuCtx, RunMode},
    strerror::StrError,
};
use ::spel_katalog_settings::{
    BubblewrapExe, ConfigDir, DllOverrides, FirejailExe, GamescopeExe, LutrisExe, Network, OnRun,
    SandboxExtras, SandboxMode, Settings, ShellExe, TermCommand, UmuRunExe, UseGamescope, YmlDir,
};
use ::spel_katalog_sink::SinkIdentity;
use ::tap::{Pipe, TapOptional};

use crate::{
    App, Message, QuickMessage, Safety, oneshot_broadcast::oneshot_broadcast,
    run_game::run_script::BatchView,
};

mod run_script;

#[derive(Debug, ::thiserror::Error)]
enum ScriptGatherError {
    /// Forwarded io error.
    #[error(transparent)]
    Io(#[from] ::std::io::Error),
    /// Lua error.
    #[error(transparent)]
    Lua(#[from] LuaError),
    /// Error reading lua script.
    #[error("clould not read file {1:?}\n{0}")]
    ReadLuaScript(#[source] ::std::io::Error, PathBuf),
}

#[derive(Debug, ::thiserror::Error)]
#[error("lua error occured\n{0}")]
struct LuaError(String);

#[derive(Debug, ::thiserror::Error)]
enum ConfigError {
    #[error(transparent)]
    Io(#[from] ::std::io::Error),
    #[error(transparent)]
    Scan(#[from] ::yaml_rust2::ScanError),
}

async fn parse_extra_config(extra_config_path: &Path) -> Result<AdditionalConfig, String> {
    ::toml::from_str::<AdditionalConfig>(
        &::smol::fs::read_to_string(&extra_config_path)
            .await
            .map_err(|err| {
                ::log::error!("could not read {extra_config_path:?}\n{err}");
                format!("could not read {extra_config_path:?}")
            })?,
    )
    .map_err(|err| {
        ::log::error!("could not parse {extra_config_path:?}\n{err}");
        format!("could not parse {extra_config_path:?}")
    })
}

impl App {
    pub fn game_as_native(&self, game_id: GameId) -> Task<(NativeGame, Option<DynamicImage>)> {
        let Some(game) = self.games.by_id(game_id) else {
            ::log::warn!("could not find game with id {game_id}");
            return Task::none();
        };
        let thumb = game.thumb.clone();

        match &game.game {
            Game::Lutris(game) => {
                let GameId::Lutris(lutris_id) = game_id else {
                    ::log::error!(
                        "cannot convert lutris game without lutris id to native game, id: {game_id}"
                    );
                    return Task::none();
                };
                let name = game.name.clone();
                let runner = game.runner.clone();
                let hidden = game.hidden;
                let installed_at = game.installed_at;
                let yml_dir = self.settings.get::<YmlDir>();
                let configpath = format!("{yml_dir}/{}.yml", game.configpath);
                let extra_config_path = self
                    .settings
                    .get::<ConfigDir>()
                    .as_path()
                    .join("games")
                    .join(format!("{lutris_id}.toml"));

                Task::future(async move {
                    let config = ::smol::fs::read_to_string(&configpath).await?;
                    let config = lutris_config::Config::parse(&config)?;
                    let extra_config = if extra_config_path.exists() {
                        Some(
                            parse_extra_config(&extra_config_path)
                                .await
                                .map_err(|err| eyre!(err))?,
                        )
                    } else {
                        None
                    };
                    let game = LutrisCtx {
                        config: &config,
                        exe: &config.game.exe,
                        extra_config: extra_config.as_ref(),
                        name: &name,
                        runner,
                        wine_prefix: config.game.prefix.as_deref(),
                        hidden,
                        installed_at,
                        id: game_id,
                    }
                    .into_native()
                    .map_err(StrError::into_error)?;
                    let name = &game.name;
                    let thumb = thumb.as_ref().and_then(|thumb| match thumb {
                        ::iced_widget::image::Handle::Path(_, path) => ::image::open(path)
                            .map_err(|err| {
                                ::log::warn!(
                                    "failed to convert thumbnail to an image for {name:?}\n{err}"
                                )
                            })
                            .ok(),
                        ::iced_widget::image::Handle::Bytes(_, bytes) => ::image::load_from_memory(
                            bytes,
                        )
                        .map_err(|err| {
                            ::log::warn!(
                                "failed to convert thumbnail to an image for {name:?}\n{err}"
                            )
                        })
                        .ok(),
                        ::iced_widget::image::Handle::Rgba {
                            width,
                            height,
                            pixels,
                            ..
                        } => ::image::RgbaImage::from_raw(*width, *height, pixels.to_vec())
                            .tap_none(|| {
                                ::log::warn!("failed to convert thumbnail to an image for {name:?}")
                            })
                            .map(DynamicImage::from),
                    });
                    Ok((game, thumb))
                })
                .then(move |result: ::color_eyre::Result<_>| {
                    result
                        .map_err(|err| {
                            ::log::error!(
                                "could not convert lutris game {game_id} to native game\n{err}"
                            )
                        })
                        .ok()
                        .map_or_else(Task::none, Task::done)
                })
            }
            Game::Native { .. } => Task::none(),
        }
    }

    /// Get sandbox extra read-only dirs.
    fn sandbox_ro_dirs(settings: &Settings) -> Vec<PathBuf> {
        settings
            .get::<SandboxExtras>()
            .split(';')
            .map(|sb| sb.trim())
            .filter(|sb| !sb.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    /// Get global dll overrides.
    fn dll_overrides(settings: &Settings) -> Vec<String> {
        settings
            .get::<DllOverrides>()
            .split(';')
            .map(|ovr| ovr.trim())
            .filter(|ovr| !ovr.is_empty())
            .map(String::from)
            .collect()
    }

    /// Run a native game.
    pub fn run_native_game(&mut self, game: NativeGame, run_mode: RunMode) -> Task<Message> {
        let bwrap = self.settings.get::<BubblewrapExe>().clone();
        let umu = self.settings.get::<UmuRunExe>().clone();
        let shell = self.settings.get::<ShellExe>().clone();
        let term = self.settings.get::<TermCommand>().clone();
        let net_disabled = self.settings.get::<Network>().is_disabled();
        let use_gamescope = self.settings.get::<UseGamescope>().is_yes();
        let gamescope = self.settings.get::<GamescopeExe>().clone();
        let sink_builder = self.sink_builder.clone();
        let sandbox_ro_dirs = Self::sandbox_ro_dirs(&self.settings);
        let dll_overrides = Self::dll_overrides(&self.settings);

        Task::<Option<Message>>::future(async move {
            let name = game.name.clone();
            let sink_builder = sink_builder
                .with_locked_channel(|| SinkIdentity::Name(name.clone()))
                .map_err(|err| {
                    ::log::error!("could not create locked sink builder for {name}\n{err}")
                })
                .ok()?;
            let ctx = NativeUmuCtx {
                common: CommonUmuCtx {
                    bwrap: bwrap.as_path(),
                    umu: umu.as_path(),
                    shell: shell.as_path(),
                    term: &term,
                    net_disabled,
                    sink_builder,
                    dll_overrides,
                    sandbox_ro_dirs,
                    use_gamescope,
                    gamescope: gamescope.as_path(),
                    callback: Callback::default(),
                },
                config: game,
            };

            ctx.run(run_mode)
                .await
                .map_err(|err| ::log::error!("could not run game {name}\n{err}"))
                .map(Message::from)
                .ok()
        })
        .and_then(Task::done)
    }

    pub fn run_game(&mut self, id: GameId, safety: Safety, no_game: bool) -> Task<Message> {
        let Some(game) = self.games.by_id(id) else {
            status!(&self.sender, "could not run game with id {id}");
            return Task::none();
        };

        let game = match &game.game {
            Game::Lutris(lutris_game) => lutris_game,
            Game::Native { uuid, .. } => {
                let uuid = *uuid;
                let games_db = self.games_db.clone();
                let run_shell = match safety {
                    Safety::None | Safety::Sandbox => false,
                    Safety::SandboxShell => true,
                };
                return Task::<Option<Message>>::future(async move {
                    let game = ::smol::unblock(move || games_db.get_game(uuid))
                        .await
                        .map_err(|err| ::log::error!("could not game with id {uuid}\n{err}"))
                        .ok()?
                        .pipe(Box::new);

                    Some(if run_shell {
                        Message::RunShellNative(game)
                    } else {
                        Message::RunGameNative(game)
                    })
                })
                .and_then(Task::done);
            }
        };

        let lutris_id = match id {
            GameId::Lutris(lutris_id) => lutris_id,
            GameId::Native(uuid) => {
                ::log::error!("lutris game somehow gotten for uuid {uuid}");
                return Task::none();
            }
        };

        let lutris = self.settings.get::<LutrisExe>().clone();
        let firejail = self.settings.get::<FirejailExe>().clone();
        let term = self.settings.get::<TermCommand>().clone();
        let bwrap = self.settings.get::<BubblewrapExe>().clone();
        let umu = self.settings.get::<UmuRunExe>().clone();
        let shell = self.settings.get::<ShellExe>().clone();
        let gamescope = self.settings.get::<GamescopeExe>().clone();
        let use_gamescope = self.settings.get::<UseGamescope>().is_yes();
        let sandbox_mode = *self.settings.get::<SandboxMode>();
        let sandbox_ro_dirs = Self::sandbox_ro_dirs(&self.settings);
        let dll_overrides = Self::dll_overrides(&self.settings);
        let slug = game.slug.clone();
        let name = game.name.clone();
        let runner = game.runner.clone();
        let hidden = game.hidden;
        let installed_at = game.installed_at;
        let net_disabled = self.settings.get::<Network>().is_disabled();
        let sink_builder = self.sink_builder.clone();
        let yml_dir = self.settings.get::<YmlDir>();
        let configpath = format!("{yml_dir}/{}.yml", game.configpath);
        let extra_config_path = self
            .settings
            .get::<ConfigDir>()
            .as_path()
            .join("games")
            .join(format!("{lutris_id}.toml"));
        let script_dir = self.settings.get::<ConfigDir>().as_path().join("scripts");

        let (send_open, recv_open) = oneshot_broadcast();

        let lua_vt = self.lua_vt();

        let cmd_task = Task::future(async move {
            let config = async {
                let config = ::smol::fs::read_to_string(&configpath).await?;
                let config = lutris_config::Config::parse(&config)?;
                Ok::<_, ConfigError>(config)
            };

            let config = match config.await {
                Ok(config) => config,
                Err(err) => {
                    ::log::error!("while loading config {configpath:?}\n{err}");
                    return "could not load config for game".to_owned().into();
                }
            };

            let extra_config = if extra_config_path.exists() {
                match parse_extra_config(&extra_config_path).await {
                    Err(err) => return err.into(),
                    Ok(additional) => Some(additional),
                }
            } else {
                None
            };

            let view = BatchView {
                id: lutris_id,
                slug: &slug,
                name: &name,
                runner: &runner,
                config: &configpath,
                extra: extra_config.as_ref(),
                hidden,
            };

            if let Err(err) =
                self::run_script::run_script(script_dir, view, &sink_builder, lua_vt).await
            {
                ::log::error!("failure when gathering/runnings scripts\n{err}");
                return "running scripts failed".to_owned().into();
            }

            let rungame = if no_game {
                None
            } else {
                Some(format!("lutris:rungameid/{lutris_id}"))
            };

            fn wl(p: impl AsRef<OsStr>) -> OsString {
                let mut s = OsString::new();
                s.push("--whitelist=");
                s.push(p);
                s
            }

            let (stdout, stderr) =
                match sink_builder.build_double(|| SinkIdentity::GameId(lutris_id)) {
                    Ok([stdout, stderr]) => (stdout, stderr),
                    Err(err) => {
                        ::log::error!("could not create process output sinks\n{err}");
                        return "could not create output sinks".to_owned().into();
                    }
                };

            let cmd = match (safety, sandbox_mode) {
                (Safety::None, _) => {
                    ::log::info!("executing {lutris:?} with arguments\n{:#?}", &[&rungame]);
                    ::smol::process::Command::new(lutris)
                        .args(rungame)
                        .kill_on_drop(true)
                        .stdout(stdout)
                        .stderr(stderr)
                        .status()
                }
                (Safety::Sandbox, SandboxMode::Firejail) => {
                    let mut args = Vec::new();
                    ::log::info!("parsed game config\n{config:#?}");
                    if let Some(additional) = extra_config
                        .map(|additional| additional.sandbox_root)
                        .filter(|roots| !roots.is_empty())
                    {
                        args.extend(additional.into_iter().map(wl));
                    } else {
                        args.push(wl(config
                            .game
                            .common_parent(|| ::spel_katalog_settings::HOME.as_path())));
                    }

                    if net_disabled {
                        args.push("--net=none".into());
                    }

                    args.push(lutris.as_os_str().into());

                    if let Some(rungame) = rungame {
                        args.push(rungame.into());
                    };

                    ::log::info!("executing {firejail:?} with arguments\n{args:#?}");

                    ::smol::process::Command::new(firejail)
                        .args(args)
                        .kill_on_drop(true)
                        .stdout(stdout)
                        .stderr(stderr)
                        .status()
                }

                (Safety::SandboxShell, SandboxMode::Firejail) => {
                    ::log::error!("shell requires sandbox mode of bubblewrap");
                    return "only bubblewrap supported for shell".to_owned().into();
                }
                (Safety::Sandbox, SandboxMode::Bubblewrap) => {
                    return LutrisUmuCtx {
                        common: CommonUmuCtx {
                            bwrap: bwrap.as_path(),
                            sink_builder,
                            term: &term,
                            umu: umu.as_path(),
                            net_disabled,
                            sandbox_ro_dirs,
                            callback: Callback::new(|| send_open.send(())),
                            shell: shell.as_path(),
                            dll_overrides,
                            gamescope: gamescope.as_path(),
                            use_gamescope,
                        },
                        lutris: LutrisCtx {
                            config: &config,
                            exe: &config.game.exe,
                            extra_config: extra_config.as_ref(),
                            name: &name,
                            runner,
                            wine_prefix: config.game.prefix.as_deref(),
                            hidden,
                            installed_at,
                            id,
                        },
                    }
                    .run()
                    .await
                    .into();
                }
                (Safety::SandboxShell, SandboxMode::Bubblewrap) => {
                    return LutrisUmuCtx {
                        common: CommonUmuCtx {
                            bwrap: bwrap.as_path(),
                            net_disabled,
                            sandbox_ro_dirs,
                            callback: Callback::new(|| send_open.send(())),
                            shell: shell.as_path(),
                            sink_builder,
                            term: &term,
                            umu: umu.as_path(),
                            dll_overrides,
                            gamescope: gamescope.as_path(),
                            use_gamescope,
                        },
                        lutris: LutrisCtx {
                            config: &config,
                            exe: &config.game.exe,
                            extra_config: extra_config.as_ref(),
                            name: &name,
                            runner,
                            wine_prefix: config.game.prefix.as_deref(),
                            hidden,
                            installed_at,
                            id,
                        },
                    }
                    .run_shell()
                    .await
                    .into();
                }
            };

            send_open.send(());

            match cmd.await {
                Ok(status) => format!("{name} exited with {status}").into(),
                Err(err) => {
                    ::log::error!("could not run {slug}\n{err}");
                    format!("could not run {slug}").into()
                }
            }
        });

        let to_open = *self.settings.get::<OnRun>();
        let open_process_list = Task::future(async move {
            match recv_open.recv_async().await {
                Some(_) => match to_open {
                    OnRun::Process => Some(Message::Quick(QuickMessage::OpenProcessInfo)),
                    OnRun::Info => Some(Message::Quick(QuickMessage::OpenGameInfo)),
                    OnRun::None => None,
                },
                None => {
                    ::log::error!("could not receive open signel through oneshot");
                    None
                }
            }
        })
        .then(|msg| msg.map_or_else(Task::none, Task::done));

        Task::batch([cmd_task, open_process_list])
    }
}
