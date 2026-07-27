//! Application daemon library.

use ::clap::{Args, Parser};
use ::spel_katalog_ipc::typed::{MethodResolver, create_socket, listen};

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

        ::smol::block_on(async {
            let socket = create_socket("spel-katalog-daemon-ipc", &xdg).await?;

            listen(socket, async |incoming| {
                MethodResolver::new(incoming)
                    .post(async |post| post.resource(run::run).await)
                    .await
                    .get(async |get| get.resource(children::children).await)
                    .await
                    .finish()
                    .await
            })
            .await?;
            Ok(())
        })
    }
}
