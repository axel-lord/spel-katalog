//! Implementation of run response.

use ::core::time::Duration;
use ::std::os::fd::OwnedFd;

use ::bytes::Bytes;
use ::flume::unbounded;
use ::rustix::fs::Mode;
use ::smol::{
    Timer, fs,
    future::{self, FutureExt},
    io::{self, AsyncReadExt, AsyncWriteExt},
};
use ::spel_katalog_formats::daemon;
use ::spel_katalog_ipc::{IncomingRequest, http::HttpResponse};
use ::spel_katalog_run::id_channel;
use ::spel_katalog_settings::Settings;
use ::spel_katalog_sink::SinkBuilder;
use ::tinyvec::ArrayVec;
use ::uuid::Uuid;

/// Process run request
///
/// # Errors
/// If the request is invalid, or fails to run.
pub async fn run(incoming: IncomingRequest) -> Result<Bytes, HttpResponse> {
    let body = incoming.body().await?;
    let daemon::request::RunConfig {
        config,
        run_mode,
        settings,
    } = ::serde_json::from_slice::<daemon::request::RunConfig<Settings>>(&body)?;

    let fifo_path = settings
        .xdg()
        .get_runtime_file(format!("pipe/{}", Uuid::now_v7()))?;
    if let Some(parent) = fifo_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let fifo_path = ::smol::unblock(move || -> Result<_, HttpResponse> {
        ::rustix::fs::mkfifoat(::rustix::fs::CWD, &fifo_path, Mode::RUSR | Mode::WUSR)?;
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
                let result: Result<(), io::Error> = ::smol::block_on(async move {
                    let (tx, rx) = unbounded::<ArrayVec<[u8; 128]>>();
                    let mut fifo = fs::OpenOptions::new().append(true).open(&fifo_path).await?;
                    let mut pipe = fs::File::from(OwnedFd::from(pipe_r));

                    let read_pipe = async move {
                        let mut buf = [0u8; 128];
                        let mut opt_tx = Some(tx);
                        loop {
                            let n = pipe.read(&mut buf).await?;
                            if n == 0 {
                                break;
                            }

                            if let Some(tx) = &opt_tx
                                && let Err(err) = tx.send(ArrayVec::from_array_len(buf, n))
                            {
                                ::log::warn!(
                                    "could not send buffer, converting to null writer\n{err}"
                                );
                                opt_tx = None;
                            }
                        }
                        Ok(())
                    };

                    let forward_pipe = async move {
                        loop {
                            let result = async {
                                let buf = rx.recv_async().await.map_err(io::Error::other)?;
                                fifo.write_all(&buf)
                                    .or(async {
                                        Timer::after(Duration::from_secs(1)).await;
                                        ::log::warn!(
                                            "writing to fifo stalled for > 1s, closing connection"
                                        );
                                        Err(io::Error::other("fifo write stalled for > 1s"))
                                    })
                                    .await
                            }
                            .await;
                            if let Err(err) = result {
                                ::log::warn!("closing fifo writer due to error\n{err}");
                                break;
                            }
                        }
                    };

                    future::zip(read_pipe, forward_pipe).await.0
                });
                if let Err(err) = result {
                    ::log::error!("error in pipe thread\n{err}");
                }
            }
        })?;

    let (tx, rx) = id_channel();

    ::std::thread::Builder::new()
        .name(format!("spel-katalog-run-[{trunc_name}]"))
        .spawn(move || {
            if let Some(message) =
                ::spel_katalog_run::run_native_game(config, run_mode, &settings, sink_builder, tx)
                    .and_then(::smol::block_on)
            {
                ::log::info!("game exited with message: {message}");
            };
        })?;

    let pid = rx.recv().await;
    let response = if let Some(pid) = pid {
        daemon::response::Run::CreatedPipe {
            name: trunc_name,
            path: fifo_path,
            pid: pid.into(),
        }
    } else {
        daemon::response::Run::CouldNotRun { name: trunc_name }
    };

    let response = ::serde_json::to_vec(&response)?;

    Ok(Bytes::from_owner(response))
}
