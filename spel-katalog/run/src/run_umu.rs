//! Run games using bubblewrap and umu.

use ::std::{
    collections::BTreeSet,
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use ::color_eyre::{Section, eyre::eyre};
use ::rustc_hash::FxHashMap;
use ::smol::process::Command;
use ::spel_katalog_formats::{
    AdditionalConfig, Bind, GameId, LutrisRunner, NativeGame, NativeRunner, RunMode, Timestamp,
    lutris_config,
};
use ::spel_katalog_sink::SinkBuilder;
use ::tap::Pipe;

use crate::{Callback, macros::args};

/// Context needed to run game with bubblewrap and umu.
#[derive(Debug)]
pub struct CommonUmuCtx<'a> {
    /// Path to `bwrap` executable.
    pub bwrap: &'a Path,
    /// Path to `umu-run` executable.
    pub umu: &'a Path,
    /// Path to shell executable.
    pub shell: &'a Path,
    /// Command line to prefix command with
    /// to start in a terminal.
    pub term: &'a str,
    /// Is net disabled.
    pub net_disabled: bool,
    /// Global dll overrides.
    pub dll_overrides: Vec<String>,
    /// Global sandbox read only additions.
    pub sandbox_ro_dirs: Vec<PathBuf>,
    /// Should gamescope be used.
    pub use_gamescope: bool,
    /// Path to gamescope executable.
    pub gamescope: &'a Path,
    /// Callback used to signal game was started.
    pub callback: Callback,
    /// Sink builder to use for command outputs.
    pub sink_builder: SinkBuilder,
}

/// Context needed to run native games.
#[derive(Debug)]
pub struct NativeUmuCtx<'a> {
    /// Common context.
    pub common: CommonUmuCtx<'a>,
    /// Game config.
    pub config: NativeGame,
}

/// If possible bind user in wine prefix to steamuser in umu prefix.
fn bind_user(wine_prefix: &Path, umu_prefix: &Path) -> Option<Bind> {
    const USERS: &str = "drive_c/users";
    let username = ::users::get_current_username()?;

    let wine_home = wine_prefix.join(USERS).join(&username);
    if !wine_home.exists() {
        return None;
    }

    let umu_home = umu_prefix.join(USERS).join("steamuser");

    Some(Bind::asymmetric(wine_home, umu_home))
}

/// Initialize umu prefix.
async fn init_umu_prefix(
    umu: &Path,
    umu_prefix: &Path,
    verbs: &[String],
    drives: &mut (dyn '_ + Send + Sync + Iterator<Item = (char, &Path)>),
    sink_builder: SinkBuilder,
    envs: &FxHashMap<String, String>,
) -> ::color_eyre::Result<()> {
    let [stdout, stderr] = sink_builder.build(|| "Init Prefix")?;
    let status = Command::new(umu)
        .stdout(stdout)
        .stderr(stderr)
        .args(
            ["winetricks", "fontsmooth=rgb"]
                .into_iter()
                .chain(verbs.iter().map(String::as_str)),
        )
        .envs(envs)
        .env("WINEPREFIX", umu_prefix)
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|err| {
            ::log::error!("could not run command to create umu prefix {umu_prefix:?}, {err}");
            eyre!(err).note("could not create umu prefix")
        })?;

    if !status.success() {
        ::log::error!("failed to create umu prefix {status:?}");
        return Err(eyre!("could not create umu prefix"));
    }

    for (letter, link) in drives {
        if let Err(err) =
            ::smol::fs::unix::symlink(link, umu_prefix.join(format!("dosdevices/{letter}:"))).await
            && !matches!(err.kind(), ErrorKind::AlreadyExists)
        {
            ::log::warn!("could not crate device {letter}: in {umu_prefix:?}\n{err}");
        }
    }

    Ok(())
}

/// Split terminal command line into executable and arguments.
fn term_path(term: &str) -> ::color_eyre::Result<(PathBuf, Vec<PathBuf>)> {
    let term_command = ::shell_words::split(term)
        .map_err(|err| eyre!(err).note(format!("could not split {term}")))?;
    let [term, term_args @ ..] = term_command.as_slice() else {
        return Err(eyre!("cannot get command from {term:?}"));
    };

    Ok((
        PathBuf::from(term),
        term_args.iter().map(PathBuf::from).collect(),
    ))
}

impl NativeUmuCtx<'_> {
    /// Run shell in prefix.
    ///
    /// # Errors
    /// If context cannot run shell.
    pub async fn run_shell(self) -> ::color_eyre::Result<String> {
        self.run(RunMode::Shell).await
    }

    /// Run game.
    ///
    /// # Errors
    /// If context cannot run.
    pub async fn run_game(self) -> ::color_eyre::Result<String> {
        self.run(RunMode::Exe).await
    }

    /// Initizlize prefix.
    ///
    /// # Errors
    /// If context cannot initialize prefix.
    pub async fn run_init(self) -> ::color_eyre::Result<String> {
        self.run(RunMode::Init).await
    }

    /// Run context.
    ///
    /// # Errors
    /// If context cannot run given mode.
    pub async fn run(self, run_mode: RunMode) -> ::color_eyre::Result<String> {
        let NativeUmuCtx {
            common:
                CommonUmuCtx {
                    bwrap,
                    umu,
                    shell,
                    term,
                    net_disabled,
                    dll_overrides: global_dll_override,
                    sandbox_ro_dirs: global_ro_bind,
                    callback: send_open,
                    use_gamescope: global_use_gamescope,
                    gamescope,
                    sink_builder,
                },
            config,
        } = self;
        ::log::info!("using game config\n{config:#?}");
        let NativeGame {
            name,
            timestamp: _,
            exe,
            runner,
            prefix,
            hidden: _,
            use_net,
            env,
            attrs: _,
            drives,
            dll_override,
            wt_verb,
            bind,
            ro_bind,
            use_gamescope,
            gamescope_args,
            shadow: _,
        } = config;

        let use_gamescope = use_gamescope.unwrap_or(global_use_gamescope);
        let home = ::std::env::home_dir().ok_or_else(|| {
            ::log::error!("could not find user home directory");
            eyre!("could not find user home directory")
        })?;
        let xdg_runtime_dir = ::std::env::var_os("XDG_RUNTIME_DIR")
            .ok_or_else(|| {
                ::log::error!("could not find XDG_RUNTIME_DIR");
                eyre!("could not find XDG_RUNTIME_DIR")
            })
            .map(PathBuf::from)?;
        let umu_dir = home.join(".local/share/umu");
        let xauthority = home.join(".Xauthority");

        let mut args = Vec::<OsString>::new();
        let term_path = if run_mode.is_shell() {
            let (term, term_args) = term_path(term)?;
            args.extend(term_args.into_iter().map(OsString::from));
            args.extend(args![bwrap]);
            Some(term)
        } else {
            None
        };

        if runner.is_wine()
            && let Some(prefix) = prefix.as_deref()
            && (run_mode.is_init() || !prefix.exists())
        {
            init_umu_prefix(
                umu,
                prefix,
                &wt_verb,
                &mut drives
                    .iter()
                    .map(|(letter, link)| (*letter, link.as_path())),
                sink_builder.clone(),
                &env,
            )
            .await?;
        }

        if run_mode.is_init() {
            return Ok("prefix initialized".to_owned());
        }

        let wayland = xdg_runtime_dir.join("wayland-1");
        let pulse = xdg_runtime_dir.join("pulse");
        let bus = xdg_runtime_dir.join("bus");

        #[rustfmt::skip]
        args.extend(args![
            "--dev", "/dev",
            "--proc", "/proc",
            "--ro-bind", "/usr", "/usr",
            "--ro-bind", "/etc", "/etc",
            "--ro-bind", "/sys", "/sys",
            "--ro-bind-try", "/opt/rocm", "/opt/rocm",
            "--symlink", "/usr/lib", "/lib",
            "--symlink", "/usr/lib64", "/lib64",
            "--symlink", "/usr/lib32", "/lib32",
            "--symlink", "/usr/bin", "/bin",
            "--symlink", "/usr/bin", "/sbin",
            "--tmpfs", "/var",
            "--tmpfs", "/run",
            "--tmpfs", "/home",
            "--tmpfs", "/tmp",
            "--ro-bind-try", &bus, bus,
            "--bind-try", &pulse, pulse,
            "--bind-try", &wayland, wayland,
            "--ro-bind", &xauthority, xauthority,
            "--dev-bind", "/dev/dri", "/dev/dri",
            "--dev-bind", "/dev/snd", "/dev/snd",
            "--bind", &umu_dir, umu_dir,
            "--setenv", "PATH", "/usr/bin",
            "--hostname", "spel-katalog",
            "--die-with-parent",
            "--new-session",
            "--unshare-all",
        ]);

        if !use_gamescope {
            args.extend(args![
                "--ro-bind-try",
                "/tmp/.X11-unix/X0",
                "/tmp/.X11-unix/X0"
            ]);
        }

        for root in &global_ro_bind {
            args.extend(args!["--ro-bind-try", root, root]);
        }

        for bind in &ro_bind {
            let [src, dest] = bind.normalize();
            args.extend(args!["--ro-bind", src, dest]);
        }

        for bind in &bind {
            let [src, dest] = bind.normalize();
            args.extend(args!["--bind", src, dest]);
        }

        if use_net.unwrap_or(!net_disabled) {
            args.extend(args!["--share-net"]);
        }

        for (key, value) in &env {
            args.extend(args!["--setenv", key, value]);
        }

        if let Some(prefix) = prefix.as_deref() {
            args.extend(args!["--setenv", "WINEPREFIX", prefix]);
        }

        if !dll_override.is_empty() || !global_dll_override.is_empty() {
            let dll_overrides = dll_override
                .iter()
                .chain(&global_dll_override)
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .pipe(|i| ::itertools::Itertools::intersperse(i, ","))
                .chain(["=n,b"])
                .collect::<String>();

            args.extend(args!["--setenv", "WINEDLLOVERRIDES", dll_overrides]);
        }

        if let Some(parent) = exe.parent() {
            args.extend(args!["--chdir", parent]);
        }

        if run_mode.is_shell() {
            args.extend(args![shell]);
        } else {
            if use_gamescope {
                args.extend(args![gamescope]);
                args.extend(gamescope_args.iter().map(OsString::from));
                args.extend(args!["--"]);
            }
            if runner.is_wine() {
                args.extend(args![umu]);
            }
            args.extend(args![exe]);
        }

        let process_path = term_path.unwrap_or_else(|| bwrap.to_path_buf());
        ::log::info!("running {process_path:?} with args\n{args:#?}");
        let [stdout, stderr] = sink_builder.build(|| name.clone())?;
        let cmd = Command::new(process_path)
            .args(args)
            .stdout(stdout)
            .stderr(stderr)
            .status();

        send_open.call();

        let status = cmd.await.map_err(|err| {
            ::log::error!("could not run {name}\n{err}");
            eyre!("could not run {name}")
        })?;

        Ok(format!("{name} exited with {status}"))
    }
}

/// Context needed to run lutris games.
#[derive(Debug)]
pub struct LutrisUmuCtx<'a> {
    /// Common context.
    pub common: CommonUmuCtx<'a>,
    /// Lutris specific context.
    pub lutris: LutrisCtx<'a>,
}

/// Lutris specific context for running games.
#[derive(Debug)]
pub struct LutrisCtx<'a> {
    /// Lutris yml config of game.
    pub config: &'a lutris_config::Config,
    /// Path to game executable.
    pub exe: &'a Path,
    /// Additional config of game.
    pub extra_config: Option<&'a AdditionalConfig>,
    /// Name of game.
    pub name: &'a str,
    /// Runner used for game.
    pub runner: LutrisRunner,
    /// Wine prefix of game.
    pub wine_prefix: Option<&'a Path>,
    /// Is the game hidden.
    pub hidden: bool,
    /// When was the game installed.
    pub installed_at: i64,
    /// Id of game.
    pub id: GameId,
}

impl<'a> LutrisCtx<'a> {
    /// Convert lutris context into a [NativeGame].
    ///
    /// # Errors
    /// If lutris context is malformed in some way.
    pub fn into_native(self) -> ::color_eyre::Result<NativeGame> {
        let Self {
            config,
            exe,
            extra_config,
            name,
            runner,
            wine_prefix,
            hidden,
            installed_at,
            id,
        } = self;
        let mut bind = Vec::new();
        let additional_roots = extra_config
            .map(|extra| extra.sandbox_root.as_slice())
            .unwrap_or_default();
        if !additional_roots.is_empty() {
            bind.extend(
                additional_roots
                    .iter()
                    .map(|root| Bind::mirrored(root.into())),
            );
        } else if runner.is_wine() {
            bind.push(Bind::mirrored(
                config
                    .game
                    .common_parent(|| ::spel_katalog_settings::HOME.as_path()),
            ));
        } else if let Some(parent) = exe.parent() {
            bind.push(Bind::mirrored(parent.to_path_buf()));
        }

        let prefix = runner
            .is_wine()
            .then(|| {
                wine_prefix
                    .and_then(|prefix| prefix.parent().or_else(|| exe.parent()))
                    .map(|dir| dir.join(".umu_pfx"))
            })
            .flatten();

        if let Some(wine_prefix) = wine_prefix
            && let Some(umu_prefix) = prefix.as_deref()
            && let Some(home_bind) = bind_user(wine_prefix, umu_prefix)
        {
            bind.push(home_bind);
        }

        Ok(NativeGame {
            name: name.to_owned(),
            timestamp: Timestamp::try_from(installed_at)?,
            exe: exe.to_path_buf(),
            runner: match runner {
                LutrisRunner::Wine => NativeRunner::Wine,
                LutrisRunner::Linux => NativeRunner::Linux,
                LutrisRunner::Other(runner) => {
                    return Err(eyre!("unknown runner {runner} for {name}"));
                }
            },
            prefix,
            hidden,
            use_net: None,
            env: config.system.env.clone(),
            attrs: extra_config
                .map(|extra| extra.attrs.clone())
                .unwrap_or_default(),
            drives: FxHashMap::from_iter([('g', PathBuf::from("../.."))]),
            dll_override: Vec::new(),
            wt_verb: Vec::new(),
            bind,
            ro_bind: Vec::new(),
            use_gamescope: None,
            gamescope_args: Vec::new(),
            shadow: Some(id),
        })
    }
}

impl<'a> LutrisUmuCtx<'a> {
    /// Run shell in prefix.
    ///
    /// # Errors
    /// If context cannot run shell.
    pub async fn run_shell(self) -> ::color_eyre::Result<String> {
        self.into_native()?.run_shell().await
    }

    /// Run game.
    ///
    /// # Errors
    /// If context cannot run.
    pub async fn run(self) -> ::color_eyre::Result<String> {
        self.into_native()?.run_game().await
    }

    /// Convert into a native game run context.
    ///
    /// # Errors
    /// If the lutris context cannot produce a native context.
    pub fn into_native(self) -> ::color_eyre::Result<NativeUmuCtx<'a>> {
        let Self { common, lutris } = self;
        Ok(NativeUmuCtx {
            common,
            config: lutris.into_native()?,
        })
    }
}
