//! Module used for running games.

use ::core::{convert::Infallible, fmt::Debug};
use ::std::{
    io::PipeReader,
    os::fd::OwnedFd,
    path::{Path, PathBuf},
};

use ::smol::io::{AsyncReadExt, AsyncWriteExt};
use ::spel_katalog_formats::{NativeGame, RunMode};
use ::spel_katalog_settings::{
    BubblewrapExe, DllOverrides, GamescopeExe, Network, SandboxExtras, Settings, ShellExe,
    TermCommand, UmuRunExe, UseGamescope,
};
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity, SinkWriter};
use ::tap::Conv;
use ::unicode_segmentation::UnicodeSegmentation;

use crate::run_umu::{CommonUmuCtx, NativeUmuCtx};

mod macros;
pub mod run_umu;

/// Get log directory if available.
fn log_dir(xdg: &::xdg::BaseDirectories) -> Option<PathBuf> {
    xdg.get_runtime_file("logs")
        .map_err(|err| ::log::error!("could not get runtime directory\n{err}"))
        .ok()
}

/// Get sandbox extra read-only dirs.
pub fn sandbox_ro_dirs(settings: &Settings) -> Vec<PathBuf> {
    settings
        .get::<SandboxExtras>()
        .split(';')
        .map(|sb| sb.trim())
        .filter(|sb| !sb.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Get global dll overrides.
pub fn dll_overrides(settings: &Settings) -> Vec<String> {
    settings
        .get::<DllOverrides>()
        .split(';')
        .map(|ovr| ovr.trim())
        .filter(|ovr| !ovr.is_empty())
        .map(String::from)
        .collect()
}

/// Get stdout and stderr file handles.
async fn sink_proxy(log_dir: &Path, name: &str, sink_builder: SinkBuilder) -> Option<SinkBuilder> {
    let when = ::spel_katalog_formats::Timestamp::now();
    ::smol::fs::create_dir_all(&log_dir)
        .await
        .map_err(|err| ::log::error!("could not create {log_dir:?}\n{err}"))
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
            .writers(|| SinkIdentity::Name(trunc_name.clone()))
            .map_err(|err| ::log::error!("could not create sink pipes for {trunc_name}\n{err}"))
            .ok()?;

        async fn split_copy(
            r: PipeReader,
            w1: SinkWriter,
            mut w2: ::smol::fs::File,
        ) -> ::std::io::Result<()> {
            let mut w1 = w1.into_async();
            let mut r = r.conv::<OwnedFd>().conv::<smol::fs::File>();

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
                    let stdout_task = split_copy(stdout_reader, stdout_writer, stdout_log);
                    let stderr_task = split_copy(stderr_reader, stderr_writer, stderr_log);

                    let (r1, r2) = ::smol::future::zip(stdout_task, stderr_task).await;

                    _ = r1.map_err(|err| ::log::error!("error copying stdout\n{err}"));
                    _ = r2.map_err(|err| ::log::error!("error copying stderr\n{err}"));

                    None
                })
            })
            .map_err(|err| ::log::error!("could not spawn pipe writer thread\n{err}"))
            .ok()?;

        Some([stdout, stderr].into())
    })
    .await
}

/// Run a native game.
pub fn run_native_game(
    game: NativeGame,
    run_mode: RunMode,
    settings: &Settings,
    sink_builder: SinkBuilder,
) -> Option<impl 'static + Future<Output = Option<String>>> {
    let bwrap = settings.get::<BubblewrapExe>().clone();
    let umu = settings.get::<UmuRunExe>().clone();
    let shell = settings.get::<ShellExe>().clone();
    let term = settings.get::<TermCommand>().clone();
    let net_disabled = settings.get::<Network>().is_disabled();
    let use_gamescope = settings.get::<UseGamescope>().is_yes();
    let gamescope = settings.get::<GamescopeExe>().clone();
    let sandbox_ro_dirs = sandbox_ro_dirs(settings);
    let dll_overrides = dll_overrides(settings);
    let log_dir = log_dir(settings.xdg())?;

    Some(async move {
        let name = game.name.clone();
        let sink_builder = sink_proxy(&log_dir, &name, sink_builder).await?;

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
                sink_builder,
                gamescope: gamescope.as_path(),
                callback: Callback::default(),
            },
            config: game,
        };

        ctx.run(run_mode)
            .await
            .map_err(|err| ::log::error!("could not run game {name}\n{err}"))
            .ok()
    })
}

/// Wrapper for functor called when and if a game is ran.
#[derive(Default)]
pub struct Callback {
    /// Boxed callback.
    callback: Option<Box<dyn Send + FnOnce()>>,
}

impl Callback {
    /// Construct a new instance from a callback.
    pub fn new(callback: impl 'static + Send + FnOnce()) -> Self {
        Self {
            callback: Some(Box::new(callback)),
        }
    }

    /// Call callback consuming instance.
    pub fn call(self) {
        if let Self {
            callback: Some(callback),
        } = self
        {
            callback()
        }
    }
}

impl Debug for Callback {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.write_str("OnRun")
    }
}
