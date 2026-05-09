use ::std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use ::iced_runtime::Task;
use ::spel_katalog_common::status;
use ::spel_katalog_formats::AdditionalConfig;
use ::spel_katalog_info::formats;
use ::spel_katalog_settings::{
    BubblewrapExe, ConfigDir, FirejailExe, LutrisExe, Network, OnRun, SandboxExtras, SandboxMode,
    ShellExe, TermCommand, UmuRunExe, YmlDir,
};
use ::spel_katalog_sink::SinkIdentity;

use crate::{
    App, Message, QuickMessage, Safety,
    oneshot_broadcast::oneshot_broadcast,
    run_game::{
        run_script::BatchView,
        run_umu::{UmuCtx, umu_run},
    },
};

mod macros;
mod run_script;
mod run_umu;
mod strerror;

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
    pub fn run_game(&mut self, id: i64, safety: Safety, no_game: bool) -> Task<Message> {
        let Some(game) = self.games.by_id(id) else {
            status!(&self.sender, "could not run game with id {id}");
            return Task::none();
        };

        let lutris = self.settings.get::<LutrisExe>().clone();
        let firejail = self.settings.get::<FirejailExe>().clone();
        let term = self.settings.get::<TermCommand>().clone();
        let bwrap = self.settings.get::<BubblewrapExe>().clone();
        let umu = self.settings.get::<UmuRunExe>().clone();
        let shell = self.settings.get::<ShellExe>().clone();
        let sandbox_mode = *self.settings.get::<SandboxMode>();
        let sandbox_extras = self.settings.get::<SandboxExtras>().clone();
        let slug = game.slug.clone();
        let name = game.name.clone();
        let runner = game.runner.clone();
        let hidden = game.hidden;
        let is_net_disabled = self.settings.get::<Network>().is_disabled();
        let sink_builder = self.sink_builder.clone();
        let yml_dir = self.settings.get::<YmlDir>();
        let configpath = format!("{yml_dir}/{}.yml", game.configpath);
        let extra_config_path = self
            .settings
            .get::<ConfigDir>()
            .as_path()
            .join("games")
            .join(format!("{id}.toml"));
        let script_dir = self.settings.get::<ConfigDir>().as_path().join("scripts");

        let (send_open, recv_open) = oneshot_broadcast();

        let lua_vt = self.lua_vt();

        let cmd_task = Task::future(async move {
            let config = async {
                let config = ::smol::fs::read_to_string(&configpath).await?;
                let config = formats::Config::parse(&config)?;
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
                id,
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
                Some(format!("lutris:rungameid/{id}"))
            };

            fn wl(p: impl AsRef<OsStr>) -> OsString {
                let mut s = OsString::new();
                s.push("--whitelist=");
                s.push(p);
                s
            }

            let (stdout, stderr) = match sink_builder.build_double(|| SinkIdentity::GameId(id)) {
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
                        args.push(wl(config.game.common_parent()));
                    }

                    if is_net_disabled {
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
                    return umu_run(
                        UmuCtx {
                            bwrap: bwrap.as_path(),
                            config: &config,
                            exe: &config.game.exe,
                            extra_config: extra_config.as_ref(),
                            is_net_disabled,
                            name: &name,
                            runner,
                            sandbox_extras: &sandbox_extras,
                            send_open,
                            shell: shell.as_path(),
                            slug: &slug,
                            stderr,
                            stdout,
                            term: &term,
                            umu: umu.as_path(),
                            wine_prefix: config.game.prefix.as_deref(),
                        },
                        false,
                    )
                    .await
                    .into();
                }
                (Safety::SandboxShell, SandboxMode::Bubblewrap) => {
                    return umu_run(
                        UmuCtx {
                            bwrap: bwrap.as_path(),
                            config: &config,
                            exe: &config.game.exe,
                            extra_config: extra_config.as_ref(),
                            is_net_disabled,
                            name: &name,
                            runner,
                            sandbox_extras: &sandbox_extras,
                            send_open,
                            shell: shell.as_path(),
                            slug: &slug,
                            stderr,
                            stdout,
                            term: &term,
                            umu: umu.as_path(),
                            wine_prefix: config.game.prefix.as_deref(),
                        },
                        true,
                    )
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
