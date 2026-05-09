use ::std::{ffi::OsString, path::Path, process::Stdio};

use ::spel_katalog_formats::{AdditionalConfig, Runner};
use ::spel_katalog_info::formats::Config;

use crate::{
    oneshot_broadcast::Sender,
    run_game::{
        macros::{args, strerror},
        strerror::StrError,
    },
};

#[derive(Debug)]
pub struct UmuCtx<'a> {
    pub bwrap: &'a Path,
    pub config: &'a Config,
    pub exe: &'a Path,
    pub extra_config: Option<&'a AdditionalConfig>,
    pub is_net_disabled: bool,
    pub name: &'a str,
    pub runner: Runner,
    pub sandbox_extras: &'a str,
    pub send_open: Sender<()>,
    pub shell: &'a Path,
    pub slug: &'a str,
    pub stderr: Stdio,
    pub stdout: Stdio,
    pub term: &'a str,
    pub umu: &'a Path,
    pub wine_prefix: Option<&'a Path>,
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

pub async fn umu_run(ctx: UmuCtx<'_>, run_shell: bool) -> Result<String, StrError> {
    let UmuCtx {
        bwrap,
        config,
        exe,
        extra_config,
        is_net_disabled,
        name,
        runner,
        sandbox_extras,
        send_open,
        shell,
        slug,
        stderr,
        stdout,
        term,
        umu,
        wine_prefix,
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

    let mut args = Vec::<OsString>::new();

    let term_command;
    let term_path: Option<&Path>;
    if run_shell {
        term_command =
            ::shell_words::split(term).map_err(|err| strerror!("could not split {term}, {err}"))?;
        let [term, term_args @ ..] = term_command.as_slice() else {
            return Err(strerror!("cannot get command from {term:?}"));
        };
        args.extend(term_args.iter().map(OsString::from));
        args.extend(args![bwrap]);
        term_path = Some(Path::new(term));
    } else {
        term_path = None;
    }

    if !umu_prefix.exists() && config.game.prefix.is_some() {
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
        "--setenv", "WINEDLLOVERRIDES", "steam_api64,version,winhttp=n,b",
        "--hostname", "games",
        "--die-with-parent",
        "--new-session",
        "--unshare-all",
    ]);

    for root in sandbox_extras
        .split(';')
        .filter(|s| !s.is_empty())
        .map(Path::new)
        .filter(|p| p.exists())
    {
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

    if !is_net_disabled {
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
        let dir_device = umu_prefix.join("dosdevices").join("g:");
        args.extend(args!["--symlink", "../..", dir_device]);
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

    let cmd = ::smol::process::Command::new(if let Some(term_path) = term_path {
        term_path
    } else {
        bwrap
    })
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
