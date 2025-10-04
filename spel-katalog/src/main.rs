use ::std::sync::mpsc::channel;

use ::clap::Parser;
use ::spel_katalog::{App, Cli, exit_channel};
use ::spel_katalog_terminal::{Channels, SinkBuilder, line_channel};

fn main() -> ::color_eyre::Result<()> {
    ::color_eyre::install()?;
    let cli = Cli::parse();
    let mut log_builder = ::env_logger::builder();
    let log_builder = [
        "spel_katalog",
        "spel_katalog_batch",
        "spel_katalog_common",
        "spel_katalog_games",
        "spel_katalog_info",
        "spel_katalog_parse",
        "spel_katalog_script",
        "spel_katalog_settings",
        "spel_katalog_terminal",
    ]
    .into_iter()
    .fold(&mut log_builder, |builder, module| {
        builder.filter_module(module, ::log::LevelFilter::Debug)
    });

    if cli.advanced_terminal {
        let (log_tx, log_rx) = line_channel();
        log_builder
            .target(::env_logger::Target::Pipe(Box::new(log_tx)))
            .init();

        let (pipe_tx, pipe_rx) = channel();
        let (exit_tx, exit_rx) = exit_channel();

        let app_handle = ::std::thread::Builder::new()
            .name("spel-katalog-app".to_owned())
            .spawn(|| {
                ::spel_katalog_terminal::tui(Channels {
                    exit_tx: Box::new(|| exit_tx.send()),
                    pipe_rx,
                    log_rx,
                })
            })?;

        App::run(cli, SinkBuilder::CreatePipe(pipe_tx), Some(exit_rx))?;

        match app_handle.join() {
            Ok(result) => result?,
            Err(payload) => ::std::panic::resume_unwind(payload),
        }
        Ok(())
    } else {
        log_builder.init();
        App::run(cli, SinkBuilder::Inherit, None)
    }
}
