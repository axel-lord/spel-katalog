//! Run games using bubblewrap and umu.

use ::std::{
    collections::BTreeSet,
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Stdio,
};

use ::rustc_hash::FxHashMap;
use ::smol::process::Command;
use ::spel_katalog_formats::{
    AdditionalConfig, Bind, LutrisRunner, NativeGame, NativeRunner, Timestamp, lutris_config,
};

use crate::{
    Callback,
    macros::{args, strerror},
    strerror::StrError,
};

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
    /// Where to redirect errors.
    pub stderr: Stdio,
    /// Where to redirect output.
    pub stdout: Stdio,
    /// Global dll overrides.
    pub dll_overrides: Vec<String>,
    /// Global sandbox read only additions.
    pub sandbox_ro_dirs: Vec<PathBuf>,
    /// Callback used to signal game was started.
    pub callback: Callback,
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
        ::log::info!("user {username:?} not found in wine prefix {wine_prefix:?}");
        return None;
    }

    let umu_home = umu_prefix.join(USERS).join("steamuser");

    ::log::info!("binding {wine_home:?} to {umu_home:?}");

    Some(Bind::AsymNamed {
        src: wine_home,
        dest: umu_home,
    })
}

/// Initialize umu prefix.
async fn init_umu_prefix(
    umu: &Path,
    umu_prefix: &Path,
    verbs: &[String],
    dll_override: &mut (dyn '_ + Send + Sync + Iterator<Item = &str>),
    drives: &mut (dyn '_ + Send + Sync + Iterator<Item = (char, &Path)>),
) -> Result<(), StrError> {
    let status = Command::new(umu)
        .args(
            ["winetricks", "fontsmooth=rgb"]
                .into_iter()
                .chain(verbs.iter().map(String::as_str)),
        )
        .env("WINEPREFIX", umu_prefix)
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

    let dll_overrides = dll_override.collect::<BTreeSet<_>>();

    for dll_override in dll_overrides {
        add_dll_override(dll_override, umu_prefix).await;
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

/// Add a dell override to prefix.
async fn add_dll_override(dll_override: &str, umu_prefix: &Path) {
    ::log::info!("adding dll override {dll_override:?}");
    let status = Command::new("wine")
        .args(args![
            "reg",
            "add",
            r"HKCU\Software\Wine\DllOverrides",
            "/f",
            "/v",
            dll_override,
            "/d",
            "native,builtin"
        ])
        .env("WINEPREFIX", umu_prefix)
        .env("WINEDEBUG", "-all")
        .kill_on_drop(true)
        .status()
        .await
        .map_err(|err| ::log::error!("failed to add dll override {dll_override:?}\n{err}"));

    if let Ok(status) = status
        && !status.success()
    {
        ::log::error!(
            "could not add dll override {dll_override:?}, wine exited with status {status}"
        );
    }
}

/// Split terminal command line into executable and arguments.
fn term_path(term: &str) -> Result<(PathBuf, Vec<PathBuf>), StrError> {
    let term_command =
        ::shell_words::split(term).map_err(|err| strerror!("could not split {term}, {err}"))?;
    let [term, term_args @ ..] = term_command.as_slice() else {
        return Err(strerror!("cannot get command from {term:?}"));
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
    pub async fn run_shell(self) -> Result<String, StrError> {
        self.run_(true).await
    }

    /// Run game.
    ///
    /// # Errors
    /// If context cannot run.
    pub async fn run(self) -> Result<String, StrError> {
        self.run_(false).await
    }

    /// Run context.
    async fn run_(self, run_shell: bool) -> Result<String, StrError> {
        let NativeUmuCtx {
            common:
                CommonUmuCtx {
                    bwrap,
                    umu,
                    shell,
                    term,
                    net_disabled,
                    stderr,
                    stdout,
                    dll_overrides: global_dll_override,
                    sandbox_ro_dirs: global_ro_bind,
                    callback: send_open,
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
            net,
            env,
            attrs: _,
            drives,
            dll_override,
            wt_verb,
            bind,
            ro_bind,
        } = config;

        let home = ::std::env::home_dir().ok_or_else(|| {
            ::log::error!("could not find user home directory");
            StrError("could not find user home directory".to_owned())
        })?;
        let umu_dir = home.join(".local/share/umu");
        let xauthority = home.join(".Xauthority");

        let mut args = Vec::<OsString>::new();
        let term_path = if run_shell {
            let (term, term_args) = term_path(term)?;
            args.extend(term_args.into_iter().map(OsString::from));
            args.extend(args![bwrap]);
            Some(term)
        } else {
            None
        };

        if runner.is_wine()
            && let Some(prefix) = prefix.as_deref()
            && prefix.exists()
        {
            init_umu_prefix(
                umu,
                prefix,
                &wt_verb,
                &mut global_dll_override
                    .iter()
                    .chain(&dll_override)
                    .map(String::as_str),
                &mut drives
                    .iter()
                    .map(|(letter, link)| (*letter, link.as_path())),
            )
            .await?;
        }

        #[rustfmt::skip]
        args.extend(args![
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

        if net.unwrap_or(!net_disabled) {
            args.extend(args!["--share-net"]);
        }

        for (key, value) in &env {
            args.extend(args!["--setenv", key, value]);
        }

        if let Some(prefix) = prefix.as_deref() {
            args.extend(args!["--setenv", "WINEPREFIX", prefix]);
        }

        if run_shell {
            args.extend(args![shell]);
        } else {
            if runner.is_wine() {
                args.extend(args![umu]);
            }
            args.extend(args![exe]);
        }

        let process_path = term_path.unwrap_or_else(|| bwrap.to_path_buf());
        ::log::info!("running {process_path:?} with args\n{args:#?}");
        let cmd = Command::new(process_path)
            .args(args)
            .kill_on_drop(true)
            .stdout(stdout)
            .stderr(stderr)
            .status();

        send_open.call();

        let status = cmd.await.map_err(|err| {
            ::log::error!("could not run {name}\n{err}");
            strerror!("could not run {name}")
        })?;

        Ok(format!("{name} exited with {status}"))
    }
}

/// Context needed to run lutris games.
#[derive(Debug)]
pub struct LutrisUmuCtx<'a> {
    /// Common context.
    pub common: CommonUmuCtx<'a>,
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
}

impl<'a> LutrisUmuCtx<'a> {
    /// Run shell in prefix.
    ///
    /// # Errors
    /// If context cannot run shell.
    pub async fn run_shell(self) -> Result<String, StrError> {
        self.into_native()?.run_shell().await
    }

    /// Run game.
    ///
    /// # Errors
    /// If context cannot run.
    pub async fn run(self) -> Result<String, StrError> {
        self.into_native()?.run().await
    }

    /// Convert into a native game run context.
    ///
    /// # Errors
    /// If the lutris context cannot produce a native context.
    pub fn into_native(self) -> Result<NativeUmuCtx<'a>, StrError> {
        let LutrisUmuCtx {
            common,
            config,
            exe,
            extra_config,
            name,
            runner,
            wine_prefix,
            hidden,
            installed_at,
        } = self;

        let mut bind = Vec::new();
        let additional_roots = extra_config
            .map(|extra| extra.sandbox_root.as_slice())
            .unwrap_or_default();
        if !additional_roots.is_empty() {
            bind.extend(additional_roots.iter().map(|root| Bind::MirrorNamed {
                src: PathBuf::from(root),
            }));
        } else if runner.is_wine() {
            bind.push(Bind::MirrorNamed {
                src: config
                    .game
                    .common_parent(|| ::spel_katalog_settings::HOME.as_path()),
            });
        } else if let Some(parent) = exe.parent() {
            bind.push(Bind::MirrorNamed {
                src: parent.to_path_buf(),
            });
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

        Ok(NativeUmuCtx {
            common,
            config: NativeGame {
                name: name.to_owned(),
                timestamp: Timestamp::try_from(installed_at)?,
                exe: exe.to_path_buf(),
                runner: match runner {
                    LutrisRunner::Wine => NativeRunner::Wine,
                    LutrisRunner::Linux => NativeRunner::Linux,
                    LutrisRunner::Other(runner) => {
                        return Err(strerror!("unknown runner {runner} for {name}"));
                    }
                },
                prefix,
                hidden,
                net: None,
                env: config.system.env.clone(),
                attrs: extra_config
                    .map(|extra| extra.attrs.clone())
                    .unwrap_or_default(),
                drives: FxHashMap::from_iter([('g', PathBuf::from("../.."))]),
                dll_override: Vec::new(),
                wt_verb: Vec::new(),
                bind,
                ro_bind: Vec::new(),
            },
        })
    }
}
