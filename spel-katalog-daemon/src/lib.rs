//! Application daemon library.

use ::core::time::Duration;
use ::std::os::fd::OwnedFd;

use ::bytes::Bytes;
use ::clap::{Args, Parser};
use ::color_eyre::eyre::eyre;
use ::rustix::fs::Mode;
use ::smol::{
    future::FutureExt,
    io::{AsyncReadExt, AsyncWriteExt},
};
use ::spel_katalog_formats::DaemonRunConfigRequest;
use ::spel_katalog_ipc::{
    generic::listen,
    http::{HttpResponse, ResponseCode},
};
use ::spel_katalog_settings::Settings;
use ::spel_katalog_sink::SinkBuilder;
use ::tinyvec::ArrayVec;
use ::uuid::Uuid;

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
                        let DaemonRunConfigRequest {
                            config,
                            run_mode,
                            settings,
                        } = ::serde_json::from_slice::<DaemonRunConfigRequest<Settings>>(&body)?;

                        let fifo_path = settings
                            .xdg()
                            .get_runtime_file(format!("pipe/{}", Uuid::now_v7()))?;
                        if let Some(parent) = fifo_path.parent() {
                            ::smol::fs::create_dir_all(parent).await?;
                        }
                        let fifo_path = ::smol::unblock(move || -> Result<_, HttpResponse> {
                            ::rustix::fs::mkfifoat(
                                ::rustix::fs::CWD,
                                &fifo_path,
                                Mode::RUSR | Mode::WUSR,
                            )?;
                            Ok(fifo_path)
                        })
                        .await?;

                        let (pipe_r, pipe_w) = ::smol::unblock(::std::io::pipe).await?;
                        let sink_builder = SinkBuilder::from(pipe_w);
                        let trunc_name = config.trunc_name();

                        ::std::thread::Builder::new()
                            .name(format!("spel-katalog-pipe-[{trunc_name}]"))
                            .spawn({
                                let fifo_path = fifo_path.clone();
                                move || {
                                    let result: Result<(), ::std::io::Error> =
                                        ::smol::block_on(async move {
                                            let (tx, rx) =
                                                ::flume::unbounded::<ArrayVec<[u8; 128]>>();
                                            let mut fifo = ::smol::fs::OpenOptions::new()
                                                .append(true)
                                                .open(&fifo_path)
                                                .await?;
                                            let mut pipe =
                                                ::smol::fs::File::from(OwnedFd::from(pipe_r));

                                            let read_pipe = async move {
                                                let mut buf = [0u8; 128];
                                                let mut opt_tx = Some(tx);
                                                loop {
                                                    let n = pipe.read(&mut buf).await?;
                                                    if n == 0 {
                                                        break;
                                                    }

                                                    if let Some(tx) = &opt_tx && let Err(err) = tx.send(ArrayVec::from_array_len(buf, n))
                                                    {
                                                        ::log::warn!("could not send buffer, converting to null writer\n{err}");
                                                        opt_tx = None;
                                                    }
                                                }
                                                Ok(())
                                            };

                                            let forward_pipe = async move {
                                                loop {
                                                    let result = async {
                                                        let buf = rx.recv_async().await.map_err(::smol::io::Error::other)?;
                                                        fifo.write_all(&buf).or(async {
                                                            ::smol::Timer::after(Duration::from_secs(1)).await;
                                                            ::log::warn!("writing to fifo stalled for > 1s, closing connection");
                                                            Err(::smol::io::Error::other("fifo write stalled for > 1s"))
                                                        }).await
                                                    }.await;
                                                    if let Err(err) = result {
                                                        ::log::warn!("closing fifo writer due to error\n{err}");
                                                        break;
                                                    }
                                                }
                                            };

                                           ::smol::future::zip(read_pipe, forward_pipe).await.0

                                        });
                                    if let Err(err) = result {
                                        ::log::error!("error in pipe thread\n{err}");
                                    }
                                }
                            })?;

                        ::std::thread::Builder::new()
                            .name(format!("spel-katalog-run-[{trunc_name}]"))
                            .spawn(move || {
                                if let Some(message) = ::spel_katalog_run::run_native_game(
                                    config,
                                    run_mode,
                                    &settings,
                                    sink_builder,
                                )
                                .and_then(::smol::block_on)
                                {
                                    ::log::info!("game exited with message: {message}");
                                };
                            })?;

                        let response = ::serde_json::to_vec(
                            &::spel_katalog_formats::DaemonRunResponse::CreatedPipe {
                                name: trunc_name,
                                path: fifo_path,
                            },
                        )?;

                        Ok(Bytes::from_owner(response))
                    }
                    _ => ResponseCode::NotFound.into(),
                }
            } else {
                ResponseCode::MethodNotAllowed.into()
            }
        }).ok_or_else(|| eyre!("could not start listener thread"))?.join().map_err(|payload| ::std::panic::resume_unwind(payload))
    }
}
