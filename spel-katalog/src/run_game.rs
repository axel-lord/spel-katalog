use ::std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use ::iced::Task;
use ::mlua::Lua;
use ::spel_katalog_batch::BatchInfo;
use ::spel_katalog_common::status;
use ::spel_katalog_formats::AdditionalConfig;
use ::spel_katalog_info::formats;
use ::spel_katalog_settings::{ConfigDir, FirejailExe, LutrisExe, Network, YmlDir};
use ::spel_katalog_sink::SinkIdentity;

use crate::{App, Message, QuickMessage, Safety};

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
    /// Spawned blocking task could not be joined.
    #[error(transparent)]
    JoinError(#[from] ::tokio::task::JoinError),
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

async fn gather_scripts(script_dir: PathBuf) -> Result<Vec<PathBuf>, ScriptGatherError> {
    if !script_dir.exists() {
        ::log::info!("no script dir, skipping");
        return Ok(Vec::new());
    }
    let mut lua_scripts = Vec::new();
    let mut stack = Vec::new();

    stack.push(script_dir);

    while let Some(dir) = stack.pop() {
        let mut dir = ::tokio::fs::read_dir(dir).await?;
        while let Some(entry) = dir.next_entry().await? {
            let ft = entry.file_type().await?;

            let path = entry.path();

            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file() {
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("lua"))
                {
                    lua_scripts.push(path);
                }
            } else {
                ::log::warn!("non file or directory path in script dir, {path:?}")
            }
        }
    }

    lua_scripts.sort_unstable();

    Ok(lua_scripts)
}

impl App {
    pub fn run_game(&mut self, id: i64, safety: Safety, no_game: bool) -> Task<Message> {
        let Some(game) = self.games.by_id(id) else {
            status!(&self.sender, "could not run game with id {id}");
            return Task::none();
        };

        let lutris = self.settings.get::<LutrisExe>().clone();
        let firejail = self.settings.get::<FirejailExe>().clone();
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

        let (send_open, recv_open) = ::tokio::sync::oneshot::channel();

        let open_process_list = Task::future(async {
            match recv_open.await {
                Ok(_) => Some(Message::Quick(QuickMessage::OpenProcessInfo)),
                Err(err) => {
                    ::log::error!("could not receive open signel through oneshot\n{err}");
                    None
                }
            }
        })
        .then(|msg| msg.map_or_else(Task::none, Task::done));

        let lua_vt = self.lua_vt();
        let task = Task::future(async move {
            let config = async {
                let config = ::tokio::fs::read_to_string(&configpath).await?;
                let config = formats::Config::parse(&config)?;

                Ok::<_, ConfigError>(config)
            };

            let config = match config.await {
                Ok(config) => config,
                Err(err) => {
                    ::log::error!("while loading config {configpath:?}\n{err}");
                    return format!("could not load config for game").into();
                }
            };

            let extra_config = if extra_config_path.exists() {
                let Some(content) = ::tokio::fs::read_to_string(&extra_config_path)
                    .await
                    .map_err(|err| ::log::error!("could not read {extra_config_path:?}\n{err}"))
                    .ok()
                else {
                    return format!("could not read {extra_config_path:?}").into();
                };
                let Some(additional) = ::toml::from_str::<AdditionalConfig>(&content)
                    .map_err(|err| ::log::error!("could not parse {extra_config_path:?}\n{err}"))
                    .ok()
                else {
                    return format!("could not parse {extra_config_path:?}").into();
                };

                Some(additional)
            } else {
                None
            };

            let scripts_result = async {
                let lua_scripts = gather_scripts(script_dir).await?;

                if !lua_scripts.is_empty() {
                    let batch_info = BatchInfo {
                        id,
                        slug: slug.clone(),
                        name: name.clone(),
                        runner: runner.to_string(),
                        config: configpath.clone(),
                        attrs: extra_config
                            .as_ref()
                            .map(|extra_config| extra_config.attrs.clone())
                            .unwrap_or_default(),
                        hidden,
                    };

                    let sink_builder = sink_builder.clone();
                    ::tokio::task::spawn_blocking(move || {
                        let scripts = lua_scripts
                            .into_iter()
                            .map(|path| match ::std::fs::read_to_string(&path) {
                                Ok(content) => Ok(content),
                                Err(err) => Err(ScriptGatherError::ReadLuaScript(err, path)),
                            })
                            .collect::<Result<Vec<_>, _>>()?;

                        let lua = Lua::new();
                        ::spel_katalog_lua::Module {
                            sink_builder: &sink_builder,
                            vt: lua_vt,
                        }
                        .register(&lua)
                        .and_then(|skeleton| {
                            let module = &skeleton.module;
                            let game = batch_info.to_lua(&lua, &skeleton.game_data)?;

                            module.set("game", game)?;

                            for script in scripts {
                                lua.load(script).exec()?;
                            }

                            Ok(())
                        })
                        .map_err(|err| LuaError(err.to_string()))?;
                        Ok::<_, ScriptGatherError>(())
                    })
                    .await??;
                }

                Ok::<_, ScriptGatherError>(())
            };

            if let Err(err) = scripts_result.await {
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

            let cmd = match safety {
                Safety::None => {
                    ::log::info!("executing {lutris:?} with arguments\n{:#?}", &[&rungame]);
                    ::tokio::process::Command::new(lutris)
                        .args(rungame)
                        .kill_on_drop(true)
                        .stdout(stdout)
                        .stderr(stderr)
                        .status()
                }
                Safety::Firejail => {
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

                    ::tokio::process::Command::new(firejail)
                        .args(args)
                        .kill_on_drop(true)
                        .stdout(stdout)
                        .stderr(stderr)
                        .status()
                }
            };

            if send_open.send(()).is_err() {
                ::log::warn!("could not send open signal through oneshot");
            }

            match cmd.await {
                Ok(status) => format!("{name} exited with {status}").into(),
                Err(err) => {
                    ::log::error!("could not run {slug}\n{err}");
                    format!("could not run {slug}").into()
                }
            }
        });

        Task::batch([task, open_process_list])
    }
}
