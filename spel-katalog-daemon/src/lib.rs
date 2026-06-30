//! Application daemon library.

use ::clap::{Args, Parser};
use ::spel_katalog_formats::NativeGame;
use ::spel_katalog_ipc::{generic::listen, http::ResponseCode};

/// Daemon responsible for starting games.
#[derive(Debug, Parser)]
pub struct Cli {
    /// Deamon args.
    #[command(flatten)]
    pub args: RunDaemon,
}

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
                    "run" => {
                        let body = incoming.body().await?;
                        let game = ::serde_json::from_slice::<NativeGame>(&body)?;
                        todo!()
                    }
                    _ => ResponseCode::NotFound.into(),
                }
            } else {
                ResponseCode::MethodNotAllowed.into()
            }
        });
        Ok(())
    }
}
