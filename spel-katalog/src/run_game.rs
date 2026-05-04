use ::core::fmt::{Arguments, Display};
use ::std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use ::iced_runtime::Task;
use ::mlua::Lua;
use ::smol::stream::StreamExt;
use ::spel_katalog_batch::BatchInfo;
use ::spel_katalog_common::status;
use ::spel_katalog_formats::{AdditionalConfig, Runner};
use ::spel_katalog_info::formats::{self, Config};
use ::spel_katalog_settings::{
    BubblewrapExe, ConfigDir, FirejailExe, LutrisExe, Network, OnRun, SandboxMode, UmuRunExe,
    YmlDir,
};
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};

use crate::{
    App, Message, QuickMessage, Safety,
    app::LuaVt,
    oneshot_broadcast::{Sender, oneshot_broadcast},
};

/// Convert inputs to an array of [OsString]
///
/// ```
/// assert_eq!(
///     args!["echo", "Hello", "World!"],
///     [OsString::from("echo"), OsString::from("Hello"), OsString::from("World!")]
/// );
/// ```
macro_rules! args {
    ($($arg:expr),* $(,)?) => {
        [$(OsString::from($arg)),*]
    };
}

/// Error type converting error to a string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct StrError(String);

impl StrError {
    /// Create formatted string errror.
    pub fn fmt(args: Arguments<'_>) -> Self {
        Self(::std::fmt::format(args))
    }
}

impl<E: ::core::error::Error> From<E> for StrError {
    fn from(value: E) -> Self {
        Self(value.to_string())
    }
}

impl Display for StrError {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        <String as Display>::fmt(&self.0, f)
    }
}

impl From<StrError> for Message {
    fn from(value: StrError) -> Self {
        value.0.into()
    }
}

macro_rules! strerror {
    ($($arg:tt)*) => {
        StrError::fmt(format_args!($($arg)*))
    };
}

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

async fn gather_scripts(script_dir: PathBuf) -> Result<Vec<PathBuf>, ScriptGatherError> {
    if !script_dir.exists() {
        ::log::info!("no script dir, skipping");
        return Ok(Vec::new());
    }
    let mut lua_scripts = Vec::new();
    let mut stack = Vec::new();

    stack.push(script_dir);

    while let Some(dir) = stack.pop() {
        let mut dir = ::smol::fs::read_dir(dir).await?;
        while let Some(entry) = dir.next().await.transpose()? {
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

#[derive(Debug, Clone, Copy)]
struct BatchView<'a> {
    id: i64,
    slug: &'a str,
    name: &'a str,
    runner: &'a Runner,
    config: &'a str,
    extra: Option<&'a AdditionalConfig>,
    hidden: bool,
}

impl From<BatchView<'_>> for BatchInfo {
    fn from(
        BatchView {
            id,
            slug,
            name,
            runner,
            config,
            extra,
            hidden,
        }: BatchView,
    ) -> Self {
        BatchInfo {
            id,
            slug: slug.to_owned(),
            name: name.to_owned(),
            runner: runner.to_string(),
            config: config.to_owned(),
            attrs: extra
                .map(|extra_config| extra_config.attrs.clone())
                .unwrap_or_default(),
            hidden,
        }
    }
}

async fn run_script(
    script_dir: PathBuf,
    batch_view: BatchView<'_>,
    sink_builder: &SinkBuilder,
    lua_vt: Arc<LuaVt>,
) -> Result<(), ScriptGatherError> {
    let lua_scripts = gather_scripts(script_dir).await?;

    if !lua_scripts.is_empty() {
        let batch_info = BatchInfo::from(batch_view);

        let sink_builder = sink_builder.clone();
        ::smol::unblock(move || {
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
        .await?;
    }

    Ok(())
}

#[derive(Debug)]
struct UmuCtx<'a> {
    slug: &'a str,
    name: &'a str,
    bwrap: &'a Path,
    exe: &'a Path,
    umu: &'a Path,
    config: &'a Config,
    extra_config: Option<&'a AdditionalConfig>,
    is_net_disabled: bool,
    stdout: Stdio,
    stderr: Stdio,
    send_open: Sender<()>,
}

async fn umu_run(ctx: UmuCtx<'_>) -> Result<String, StrError> {
    let UmuCtx {
        slug,
        name,
        bwrap,
        exe,
        umu,
        config,
        extra_config,
        is_net_disabled,
        stdout,
        stderr,
        send_open,
    } = ctx;
    let home = ::std::env::home_dir().ok_or_else(|| {
        ::log::error!("could not find user home directory");
        StrError("could not find user home directory".to_owned())
    })?;
    let directory = exe.parent().ok_or_else(|| {
        ::log::error!("executable {exe:?} has no parent");
        StrError("missing executable parent".to_owned())
    })?;
    let xauthority = home.join(".Xauthority");
    let umu_dir = home.join(".local/share/umu");
    let umu_prefix = directory.join(".umu_pfx");

    if !umu_prefix.exists() {
        let status = ::smol::process::Command::new(umu)
            .arg("")
            .kill_on_drop(true)
            .status()
            .await
            .map_err(|err| {
                ::log::error!("could not run command to create umu prefix {umu_prefix:?}, {err}");
                strerror!("could not create umu prefix")
            })?;

        if !status.success() {
            ::log::error!("failed to create umu prefix {status:?}");
            return Err(strerror!("could not create umu prefix"));
        }
    }

    #[rustfmt::skip]
    let mut args = Vec::<OsString>::from(args![
        "--dev", "/dev",
        "--proc", "/proc",
        "--ro-bind", "/usr", "/usr",
        "--ro-bind", "/etc", "/etc",
        "--ro-bind", "/var", "/var",
        "--ro-bind", "/run", "/run",
        "--ro-bind", "/sys", "/sys",
        "--ro-bind-try", "/opt/rocm", "/opt/rocm",
        "--symlink", "/usr/lib", "/lib",
        "--symlink", "/usr/lib64", "/lib64",
        "--symlink", "/usr/lib32", "/lib32",
        "--symlink", "/usr/bin", "/bin",
        "--symlink", "/usr/bin", "/sbin",
        "--tmpfs", "/home",
        "--tmpfs", "/tmp",
        "--ro-bind", "/tmp/.X11-unix/X0", "/tmp/.X11-unix/X0",
        "--ro-bind", &xauthority, xauthority,
        "--dev-bind", "/dev/dri", "/dev/dri",
        "--bind", &umu_dir, umu_dir,
        "--setenv", "PATH", "/usr/bin",
        "--hostname", "games",
        "--die-with-parent",
        "--new-session",
        "--unshare-all",
    ]);

    let additional_roots = extra_config.map_or(&[][..], |extra| extra.sandbox_root.as_slice());

    let mut directory_bound = false;
    if additional_roots.is_empty() {
        let common_parent = config.game.common_parent();
        if common_parent == directory {
            directory_bound = true;
        }
        args.extend(args!["--bind", &common_parent, common_parent]);
    } else {
        for root in additional_roots {
            if root == directory {
                directory_bound = true;
            }
            args.extend(args!["--bind", root, root]);
        }
    }
    let directory_bound = directory_bound;

    if !is_net_disabled {
        args.extend(args!["--share-net"]);
    }

    if !directory_bound {
        args.extend(args!["--bind", &directory, directory,]);
    }

    args.extend(args!["--chdir", &directory,]);

    if let Some(_prefix) = &config.game.prefix {
        args.extend(args!["--setenv", "WINEPREFIX", umu_prefix, umu,]);
    }

    args.extend(args![exe]);

    ::log::info!("running with config {args:#?}");

    let cmd = ::smol::process::Command::new(bwrap)
        .args(args)
        .kill_on_drop(true)
        .stdout(stdout)
        .stderr(stderr)
        .status();

    send_open.send(());

    let status = cmd.await.map_err(|err| {
        ::log::error!("could not run {slug}\n{err}");
        strerror!("could not run {slug}")
    })?;

    Ok(format!("{name} exited with {status}"))
}

impl App {
    pub fn run_game(&mut self, id: i64, safety: Safety, no_game: bool) -> Task<Message> {
        let Some(game) = self.games.by_id(id) else {
            status!(&self.sender, "could not run game with id {id}");
            return Task::none();
        };

        let lutris = self.settings.get::<LutrisExe>().clone();
        let firejail = self.settings.get::<FirejailExe>().clone();
        let bwrap = self.settings.get::<BubblewrapExe>().clone();
        let umu = self.settings.get::<UmuRunExe>().clone();
        let sandbox_mode = *self.settings.get::<SandboxMode>();
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

        let lua_vt = self.lua_vt();
        let task = Task::future(async move {
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

            if let Err(err) = run_script(script_dir, view, &sink_builder, lua_vt).await {
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
                (Safety::Sandbox, SandboxMode::Bubblewrap) => {
                    return umu_run(UmuCtx {
                        name: &name,
                        slug: &slug,
                        bwrap: bwrap.as_path(),
                        exe: &config.game.exe,
                        umu: umu.as_path(),
                        config: &config,
                        extra_config: extra_config.as_ref(),
                        is_net_disabled,
                        stdout,
                        stderr,
                        send_open,
                    })
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

        Task::batch([task, open_process_list])
    }
}
