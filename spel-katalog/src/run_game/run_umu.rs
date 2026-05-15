use ::std::{
    collections::BTreeSet,
    ffi::OsString,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Stdio,
};

use ::smol::process::Command;
use ::spel_katalog_formats::{AdditionalConfig, Drive, LutrisRunner, NativeGame};
use ::spel_katalog_info::formats::Config;

use crate::{
    oneshot_broadcast::Sender,
    run_game::{
        macros::{args, strerror},
        strerror::StrError,
    },
};

#[derive(Debug)]
pub struct CommonUmuCtx<'a> {
    pub bwrap: &'a Path,
    pub umu: &'a Path,
    pub shell: &'a Path,
    pub term: &'a str,
    pub net_disabled: bool,
    pub stderr: Stdio,
    pub stdout: Stdio,
    pub dll_overrides: Vec<String>,
    pub sandbox_ro_dirs: Vec<PathBuf>,
    pub send_open: Sender<()>,
}

#[derive(Debug)]
pub struct NativeUmuCtx<'a> {
    pub common: CommonUmuCtx<'a>,
    pub config: NativeGame,
}

async fn init_umu_prefix(
    umu: &Path,
    umu_prefix: &Path,
    verbs: &[String],
    dll_override: Box<dyn '_ + Send + Iterator<Item = &str>>,
    symlinks: &[Drive],
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

    for Drive { link, letter } in symlinks {
        if let Err(err) =
            ::smol::fs::unix::symlink(link, umu_prefix.join(format!("dosdevices/{letter}:"))).await
            && !matches!(err.kind(), ErrorKind::AlreadyExists)
        {
            ::log::warn!("could not crate device {letter}: in {umu_prefix:?}\n{err}");
        }
    }

    Ok(())
}

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
    #[expect(dead_code)]
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
                    send_open,
                },
            config:
                NativeGame {
                    name,
                    exe,
                    runner,
                    prefix,
                    hidden: _,
                    net,
                    anchor: _,
                    env,
                    attrs: _,
                    dll_override,
                    wt_verb,
                    bind,
                    ro_bind,
                    drive,
                },
        } = self;
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
                Box::new(
                    global_dll_override
                        .iter()
                        .chain(&dll_override)
                        .map(String::as_str),
                ),
                &drive,
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
            args.extend(args![exe]);
        }

        ::log::info!("running with config {args:#?}");

        let process_path = term_path.unwrap_or_else(|| bwrap.to_path_buf());
        let cmd = Command::new(process_path)
            .args(args)
            .kill_on_drop(true)
            .stdout(stdout)
            .stderr(stderr)
            .status();

        send_open.send(());

        let status = cmd.await.map_err(|err| {
            ::log::error!("could not run {name}\n{err}");
            strerror!("could not run {name}")
        })?;

        Ok(format!("{name} exited with {status}"))
    }
}

#[derive(Debug)]
pub struct LutrisUmuCtx<'a> {
    pub common: CommonUmuCtx<'a>,
    pub config: &'a Config,
    pub exe: &'a Path,
    pub extra_config: Option<&'a AdditionalConfig>,
    pub name: &'a str,
    pub runner: LutrisRunner,
    pub slug: &'a str,
    pub wine_prefix: Option<&'a Path>,
}

impl LutrisUmuCtx<'_> {
    pub async fn run_terminal(self) -> Result<String, StrError> {
        self.run_(true).await
    }
    pub async fn run(self) -> Result<String, StrError> {
        self.run_(false).await
    }

    async fn run_(self, run_shell: bool) -> Result<String, StrError> {
        let LutrisUmuCtx {
            common:
                CommonUmuCtx {
                    bwrap,
                    umu,
                    shell,
                    term,
                    net_disabled,
                    stderr,
                    stdout,
                    dll_overrides,
                    sandbox_ro_dirs,
                    send_open,
                },
            config,
            exe,
            extra_config,
            name,
            runner,
            slug,
            wine_prefix,
        } = self;
        let home = ::std::env::home_dir().ok_or_else(|| {
            ::log::error!("could not find user home directory");
            StrError("could not find user home directory".to_owned())
        })?;
        let directory = wine_prefix.unwrap_or(exe).parent().ok_or_else(|| {
            ::log::error!("executable {exe:?} has no parent");
            StrError("missing executable parent".to_owned())
        })?;
        let xauthority = home.join(".Xauthority");
        let umu_dir = home.join(".local/share/umu");
        let umu_prefix = directory.join(".umu_pfx");

        let mut args = Vec::<OsString>::new();

        let term_path = if run_shell {
            let (term, term_args) = term_path(term)?;
            args.extend(term_args.into_iter().map(OsString::from));
            args.extend(args![bwrap]);
            Some(term)
        } else {
            None
        };

        if runner.is_wine() && !umu_prefix.exists() {
            init_umu_prefix(
                umu,
                &umu_prefix,
                &[],
                Box::new(dll_overrides.iter().map(String::as_str)),
                &[Drive {
                    link: PathBuf::from("../.."),
                    letter: 'g',
                }],
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

        for root in sandbox_ro_dirs.iter().filter(|p| p.exists()) {
            args.extend(args!["--ro-bind", root, root]);
        }

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

        if !net_disabled {
            args.extend(args!["--share-net"]);
        }

        if !directory_bound {
            args.extend(args!["--bind", &directory, directory,]);
        }

        args.extend(
            wine_prefix
                .and_then(|pfx| bind_user(pfx, &umu_prefix))
                .into_iter()
                .flatten(),
        );

        args.extend(
            config
                .system
                .env
                .iter()
                .flat_map(|(key, value)| args!["--setenv", key, value]),
        );

        args.extend(args!["--chdir", directory]);

        if let Some(_prefix) = &config.game.prefix {
            args.extend(args!["--setenv", "WINEPREFIX", umu_prefix]);
        }

        if runner.is_wine() && !run_shell {
            args.extend(args![umu]);
        }

        if run_shell {
            args.extend(args![shell]);
        } else {
            args.extend(args![exe]);
        }

        ::log::info!("running with config {args:#?}");

        let process_path = term_path.unwrap_or_else(|| bwrap.to_path_buf());
        let cmd = Command::new(process_path)
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
}

/// If possible bind user in wine prefix to steamuser in umu prefix.
fn bind_user(wine_prefix: &Path, umu_prefix: &Path) -> Option<[OsString; 3]> {
    const USERS: &str = "drive_c/users";
    let username = ::users::get_current_username()?;

    let wine_home = wine_prefix.join(USERS).join(&username);
    if !wine_home.exists() {
        ::log::info!("user {username:?} not found in wine prefix {wine_prefix:?}");
        return None;
    }

    let umu_home = umu_prefix.join(USERS).join("steamuser");

    ::log::info!("binding {wine_home:?} to {umu_home:?}");

    Some(args!["--bind", wine_home, umu_home])
}
