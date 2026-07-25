//! Application daemon library.

use ::clap::{Args, Parser};
use ::color_eyre::eyre::eyre;
use ::spel_katalog_ipc::{generic::listen, http::ResponseCode};

/// Daemon responsible for starting games.
#[derive(Debug, Parser)]
pub struct Cli {
    /// Deamon args.
    #[command(flatten)]
    pub args: RunDaemon,
}

mod children;
mod run;

/// Run application daemon.
#[derive(Debug, Args)]
pub struct RunDaemon {}

impl RunDaemon {
    /// Run daemon.
    ///
    /// # Errors
    /// If the daemon cannot be ran.
    pub fn run(self) -> ::color_eyre::Result<()> {
        let xdg = ::xdg::BaseDirectories::with_prefix("spel-katalog");

        listen(&xdg, "spel-katalog-daemon-ipc", |incoming| async move {
            if incoming.method().is_post() {
                match incoming.uri_path() {
                    "/run" => crate::run::run(incoming).await,
                    uri => ResponseCode::NotFound
                        .with_err(format!("post uri {uri:?}"))
                        .into(),
                }
            } else if incoming.method().is_get() {
                match incoming.uri_path() {
                    "/children" => crate::children::children().await,
                    uri => ResponseCode::NotFound
                        .with_err(format!("get uri {uri:?}"))
                        .into(),
                }
            } else {
                ResponseCode::MethodNotAllowed.into()
            }
        })
        .ok_or_else(|| eyre!("could not start listener thread"))?
        .join()
        .map_err(|payload| ::std::panic::resume_unwind(payload))
    }
}
