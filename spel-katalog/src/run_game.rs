use ::core::convert::Infallible;
use ::std::{
    ffi::{OsStr, OsString},
    io::PipeReader,
    path::{Path, PathBuf},
};

use ::iced::futures::TryFutureExt;
use ::iced_runtime::Task;
use ::image::DynamicImage;
use ::smol::io::{AsyncReadExt, AsyncWriteExt};
use ::spel_katalog_common::status;
use ::spel_katalog_formats::{AdditionalConfig, Game, GameId, NativeGame, lutris_config};
use ::spel_katalog_run::{
    Callback,
    run_umu::{CommonUmuCtx, LutrisCtx, LutrisUmuCtx, NativeUmuCtx, RunMode},
};
use ::spel_katalog_settings::{
    BubblewrapExe, DllOverrides, FirejailExe, GamescopeExe, LutrisExe, Network, OnRun,
    SandboxExtras, SandboxMode, Settings, ShellExe, TermCommand, UmuRunExe, UseGamescope, YmlDir,
};
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity, SinkWriter};
use ::tap::{Pipe, TapOptional};
use ::unicode_segmentation::UnicodeSegmentation;

use crate::{App, Message, QuickMessage, Safety, oneshot_broadcast::oneshot_broadcast};

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

/// Get log directory if available.
fn log_dir(xdg: &::xdg::BaseDirectories) -> Option<PathBuf> {
    xdg.get_runtime_file("logs")
        .map_err(|err| ::log::error!("could not get runtime directory\n{err}"))
        .ok()
}

/// Get stdout and stderr file handles.
async fn io_pair(
    log_dir: &Path,
    name: &str,
    sink_builder: SinkBuilder,
) -> Option<[::std::io::PipeWriter; 2]> {
    let when = ::spel_katalog_formats::Timestamp::now();
    ::smol::fs::create_dir_all(&log_dir)
        .map_err(|err| ::log::error!("could not create {log_dir:?}\n{err}"))
        .await
        .ok()?;
    let trunc_name = name.graphemes(true).take(30).collect::<String>();
    let stdout_filename = log_dir.join(format!("{when}-{trunc_name}-stdout.log"));
    let stderr_filename = log_dir.join(format!("{when}-{trunc_name}-stderr.log"));
    let stdout_log = ::smol::fs::File::create(&stdout_filename)
        .await
        .map_err(|err| ::log::error!("could not create {stdout_filename:?}\n{err}"))
        .ok()?;
    let stderr_log = ::smol::fs::File::create(&stderr_filename)
        .await
        .map_err(|err| ::log::error!("could not create {stderr_filename:?}\n{err}"))
        .ok()?;

    ::smol::unblock(move || {
        let (stdout_reader, stdout) = ::std::io::pipe()
            .map_err(|err| ::log::error!("could not create stdout pipe {err}"))
            .ok()?;
        let (stderr_reader, stderr) = ::std::io::pipe()
            .map_err(|err| ::log::error!("could not create stderr pipe {err}"))
            .ok()?;

        let [stdout_writer, stderr_writer] = sink_builder
            .get_writer_double(|| SinkIdentity::Name(trunc_name.clone()))
            .map_err(|err| ::log::error!("could not create sink pipes for {trunc_name}\n{err}"))
            .ok()?;

        async fn split_copy(
            r: PipeReader,
            w1: SinkWriter,
            mut w2: ::smol::fs::File,
        ) -> ::std::io::Result<()> {
            let mut w1 = ::smol::Unblock::new(w1);
            let mut r = ::smol::Unblock::new(r);

            let mut buf = [0; 128];
            loop {
                let n = r.read(&mut buf).await?;
                if n == 0 {
                    break;
                }

                let buf = &buf[..n];

                let w1 = w1.write_all(buf);
                let w2 = w2.write_all(buf);

                let (r1, r2) = ::smol::future::zip(w1, w2).await;
                r1?;
                r2?;
            }

            let (r1, r2) = ::smol::future::zip(w1.flush(), w2.flush()).await;
            r1?;
            r2?;

            Ok(())
        }

        _ = ::std::thread::Builder::new()
            .name(format!("spel-katalog-pipes-{trunc_name}"))
            .spawn(move || -> Option<Infallible> {
                ::smol::block_on(async move {
                    let stdout_task = split_copy(stdout_reader, stdout_writer, stdout_log)
                        .map_err(|err| ::log::error!("error copying stdout\n{err}"));
                    let stderr_task = split_copy(stderr_reader, stderr_writer, stderr_log)
                        .map_err(|err| ::log::error!("error copying stderr\n{err}"));

                    let (_, _) = ::smol::future::zip(stdout_task, stderr_task).await;

                    None
                })
            })
            .map_err(|err| ::log::error!("could not spawn pipe writer thread\n{err}"))
            .ok()?;

        Some([stdout, stderr])
    })
    .await
}

impl App {
    pub fn game_as_native(
        &self,
        game_id: GameId,
    ) -> Option<impl 'static + Future<Output = Option<(NativeGame, Option<DynamicImage>)>>> {
        let Some(game) = self.games.by_id(game_id) else {
            ::log::warn!("could not find game with id {game_id}");
            return None;
        };
        let thumb = game.thumb.clone();

        match &game.game {
            Game::Lutris(game) => {
                let GameId::Lutris(lutris_id) = game_id else {
                    ::log::error!(
                        "cannot convert lutris game without lutris id to native game, id: {game_id}"
                    );
                    return None;
                };
                let name = game.name.clone();
                let runner = game.runner.clone();
                let hidden = game.hidden;
                let installed_at = game.installed_at;
                let yml_dir = self.settings.get::<YmlDir>();
                let configpath = format!("{yml_dir}/{}.yml", game.configpath);
                let extra_config_path = self
                    .settings
                    .xdg()
                    .get_config_file(format!("games/{lutris_id}.toml"))
                    .tap_none(|| ::log::error!("could not get games/{lutris_id} in config dir"))?;

                Some(async move {
                    let config = ::smol::fs::read_to_string(&configpath)
                        .await
                        .map_err(|err| ::log::error!("could not read {configpath:?}\n{err}"))
                        .ok()?;
                    let config = lutris_config::Config::parse(&config)
                        .map_err(|err| ::log::error!("could not parse {configpath:?}\n{err}"))
                        .ok()?;
                    let extra_config = if extra_config_path.exists() {
                        parse_extra_config(&extra_config_path)
                            .await
                            .map_err(|err| ::log::error!("could not parse extra config\n{err}"))
                            .ok()
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
                    .map_err(|err| ::log::error!("could not convert game context to native\n{err}"))
                    .ok()?;
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
                    Some((game, thumb))
                })
            }
            Game::Native { .. } => None,
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
        let sandbox_ro_dirs = Self::sandbox_ro_dirs(&self.settings);
        let dll_overrides = Self::dll_overrides(&self.settings);
        let sink_builder = self.sink_builder.clone();
        let Some(log_dir) = log_dir(self.settings.xdg()) else {
            return Task::none();
        };

        Task::<Option<Message>>::future(async move {
            let name = game.name.clone();
            let [stdout, stderr] = io_pair(&log_dir, &name, sink_builder).await?;

            let ctx = NativeUmuCtx {
                common: CommonUmuCtx {
                    bwrap: bwrap.as_path(),
                    umu: umu.as_path(),
                    shell: shell.as_path(),
                    term: &term,
                    net_disabled,
                    dll_overrides,
                    sandbox_ro_dirs,
                    use_gamescope,
                    stdout,
                    stderr,
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
        let Some(log_dir) = log_dir(self.settings.xdg()) else {
            return Task::none();
        };
        let Some(extra_config_path) = self
            .settings
            .xdg()
            .get_config_file(format!("games/{lutris_id}.toml"))
        else {
            ::log::error!("could not get games/{lutris_id}.toml in config dir");
            return Task::none();
        };

        let (send_open, recv_open) = oneshot_broadcast();

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
                (safety, SandboxMode::Bubblewrap) => {
                    let Some([stdout, stderr]) = io_pair(&log_dir, &name, sink_builder).await
                    else {
                        return "could not create stdout and stderr logs".to_owned().into();
                    };
                    let ctx = LutrisUmuCtx {
                        common: CommonUmuCtx {
                            bwrap: bwrap.as_path(),
                            term: &term,
                            umu: umu.as_path(),
                            net_disabled,
                            sandbox_ro_dirs,
                            callback: Callback::new(|| send_open.send(())),
                            shell: shell.as_path(),
                            dll_overrides,
                            gamescope: gamescope.as_path(),
                            use_gamescope,
                            stdout,
                            stderr,
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
                    };

                    return if safety.is_sandbox_shell() {
                        ctx.run_shell().await.into()
                    } else {
                        ctx.run().await.into()
                    };
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
