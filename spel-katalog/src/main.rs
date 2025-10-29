use ::std::{
    io::{IsTerminal, Read},
    sync::mpsc::channel,
};

use ::spel_katalog::{exit_channel, run as run_app};
use ::spel_katalog_cli::{Cli, Subcmd, SubcmdCallbacks};
use ::spel_katalog_sink::SinkBuilder;
use ::spel_katalog_tui::{Channels, line_channel};

fn init_log(target: Option<::env_logger::Target>) {
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
        "spel_katalog_tui",
    ]
    .into_iter()
    .fold(&mut log_builder, |builder, module| {
        builder.filter_module(module, ::log::LevelFilter::Debug)
    });

    if let Some(target) = target {
        log_builder.target(target).init();
    } else {
        log_builder.init();
    }
}

fn other() -> ::color_eyre::Result<()> {
    init_log(None);
    Ok(())
}

fn run(cli: ::spel_katalog_cli::Run) -> ::color_eyre::Result<()> {
    if cli.advanced_terminal {
        let (log_tx, log_rx) = line_channel();
        init_log(Some(::env_logger::Target::Pipe(Box::new(log_tx))));

        let (pipe_tx, pipe_rx) = channel();
        let (exit_tx, exit_rx) = exit_channel();
        let keep_terminal = cli.keep_terminal;

        let app_handle = ::std::thread::Builder::new()
            .name("spel-katalog-app".to_owned())
            .spawn(move || {
                ::spel_katalog_tui::tui(
                    Channels {
                        exit_tx: Box::new(|| exit_tx.send()),
                        pipe_rx,
                        log_rx,
                    },
                    keep_terminal,
                )
            })?;

        run_app(cli, SinkBuilder::CreatePipe(pipe_tx), Some(exit_rx))?;

        match app_handle.join() {
            Ok(result) => result?,
            Err(payload) => ::std::panic::resume_unwind(payload),
        }
        Ok(())
    } else {
        init_log(None);
        let keep_terminal = cli.keep_terminal;
        run_app(cli, SinkBuilder::Inherit, None)?;

        if keep_terminal && ::std::io::stdin().is_terminal() {
            println!("Press enter to exit...");
            let mut buf = [0u8; 1];
            ::std::io::stdin().read_exact(&mut buf)?;
        }

        Ok(())
    }
}

fn main() -> ::color_eyre::Result<()> {
    ::color_eyre::install()?;
    let cli = Cli::parse();
    let cmd = Subcmd::from(cli);
    cmd.perform(SubcmdCallbacks { run, other })
}
