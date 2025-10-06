use ::std::{
    ffi::{OsStr, OsString},
    mem,
    path::{Path, PathBuf},
};

use ::iced::{
    Task,
    futures::{TryStreamExt, stream::FuturesOrdered},
};
use ::mlua::Lua;
use ::rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};
use ::serde::Serialize;
use ::spel_katalog_batch::{BatchInfo, lua_api};
use ::spel_katalog_common::status;
use ::spel_katalog_info::formats::{self, Additional};
use ::spel_katalog_script::script_file::ScriptFile;
use ::spel_katalog_settings::{
    CacheDir, ExtraConfigDir, FirejailExe, LutrisExe, Network, ScriptConfigDir, YmlDir,
};
use ::spel_katalog_terminal::SinkIdentity;
use ::tap::Pipe;

use crate::{App, Message, QuickMessage, Safety};

#[derive(Debug, ::thiserror::Error)]
enum ScriptGatherError {
    /// Forwarded io error.
    #[error(transparent)]
    Io(#[from] ::std::io::Error),
    /// Forwarded script run error.
    #[error(transparent)]
    Script(#[from] ::spel_katalog_script::Error),
    /// Forwarded parse error.
    #[error(transparent)]
    ParseRead(#[from] ::spel_katalog_script::ReadError),
    /// Forwarded string interpolation error.
    #[error("while interpolating {1:?}\n{0}")]
    Interpolate(
        #[source] ::spel_katalog_parse::InterpolationError<String>,
        String,
    ),
    #[error(transparent)]
    Lua(#[from] LuaError),
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

#[derive(Debug, Clone, Copy)]
enum ScriptExt {
    Json,
    Toml,
    Lua,
}

impl ScriptExt {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;

        if ext.eq_ignore_ascii_case("toml") {
            Some(Self::Toml)
        } else if ext.eq_ignore_ascii_case("json") {
            Some(Self::Json)
        } else if ext.eq_ignore_ascii_case("lua") {
            Some(Self::Lua)
        } else {
            None
        }
    }
}

#[derive(Debug, Default)]
struct Gathered {
    scripts: Vec<PathBuf>,
    lua_scripts: Vec<PathBuf>,
}

async fn gather_scripts(script_dir: PathBuf) -> Result<Gathered, ScriptGatherError> {
    if !script_dir.exists() {
        ::log::info!("no script dir, skipping");
        return Ok(Gathered::default());
    }
    let mut scripts = Vec::new();
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
                match ScriptExt::from_path(&path) {
                    Some(ScriptExt::Lua) => lua_scripts.push(path),
                    Some(_) => scripts.push(path),
                    None => ::log::info!("unknown script extension of path {path:?}"),
                }
            } else {
                ::log::warn!("non file or directory path in script dir, {path:?}")
            }
        }
    }

    scripts.sort_unstable();
    lua_scripts.sort_unstable();

    Ok(Gathered {
        scripts,
        lua_scripts,
    })
}

async fn read_scripts(script_paths: Vec<PathBuf>) -> Result<Vec<ScriptFile>, ScriptGatherError> {
    script_paths
        .iter()
        .map(|path| ScriptFile::read(path))
        .collect::<FuturesOrdered<_>>()
        .try_collect::<Vec<_>>()
        .await?
        .pipe(Ok)
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
            .get::<ExtraConfigDir>()
            .as_path()
            .join(format!("{id}.toml"));
        let script_dir = self.settings.get::<ScriptConfigDir>().to_path_buf();
        let thumb_db_path = self
            .settings
            .get::<CacheDir>()
            .as_path()
            .join("thumbnails.db");
        let settings_generic = self.settings.generic();

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
                let Some(additional) = ::toml::from_str::<Additional>(&content)
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
                let Gathered {
                    scripts,
                    lua_scripts,
                } = gather_scripts(script_dir).await?;
                let mut scripts = read_scripts(scripts).await?;

                scripts
                    .par_iter_mut()
                    .try_for_each(|script| -> Result<_, ScriptGatherError> {
                        let globals = mem::take(&mut script.global);
                        script
                            .visit_strings(|s| {
                                *s =
                                    ::spel_katalog_parse::interpolate_string(s, |key| match key {
                                        "HOME" => Some(::spel_katalog_settings::HOME.to_string()),
                                        "ID" => Some(id.to_string()),
                                        "SLUG" => Some(slug.clone()),
                                        "NAME" => Some(name.clone()),
                                        "RUNNER" => Some(runner.to_string()),
                                        "HIDDEN" => Some(hidden.to_string()),
                                        "EXE" => Some(config.game.exe.display().to_string()),
                                        "PREFIX" => Some(config.game.prefix.as_ref().map_or_else(
                                            || String::new(),
                                            |pfx| pfx.display().to_string(),
                                        )),
                                        "ARCH" => {
                                            Some(config.game.arch.clone().unwrap_or_default())
                                        }
                                        key => {
                                            if let Some(global) = key.strip_prefix("GLOBAL.")
                                                && let Some(value) = globals.get(global)
                                            {
                                                value.clone().pipe(Some)
                                            } else if let Some(attr) = key.strip_prefix("ATTR.") {
                                                extra_config
                                                    .as_ref()
                                                    .and_then(|extra_config| {
                                                        extra_config.attrs.get(attr).cloned()
                                                    })
                                                    .unwrap_or_default()
                                                    .pipe(Some)
                                            } else {
                                                None
                                            }
                                        }
                                    })?;
                                Ok(())
                            })
                            .map_err(|err| {
                                ScriptGatherError::Interpolate(
                                    err,
                                    script.source.as_ref().map_or_else(
                                        || script.id().to_owned(),
                                        |source| source.display().to_string(),
                                    ),
                                )
                            })?;

                        Ok(())
                    })?;

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

                    let scripts = lua_scripts
                        .into_iter()
                        .map(|path| match ::std::fs::read_to_string(&path) {
                            Ok(content) => Ok(content),
                            Err(err) => Err(ScriptGatherError::ReadLuaScript(err, path)),
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    let lua = Lua::new();
                    batch_info
                        .serialize(::mlua::serde::Serializer::new(&lua))
                        .and_then(|game| {
                            lua.globals().set("game", game)?;
                            lua_api::register_spel_katalog(
                                &lua,
                                settings_generic,
                                thumb_db_path,
                                &sink_builder,
                            )?;

                            for script in scripts {
                                lua.load(script).exec()?;
                            }

                            Ok(())
                        })
                        .map_err(|err| LuaError(err.to_string()))?;
                }

                ScriptFile::run_all(&scripts, &sink_builder).await?;
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
