use ::spel_katalog_sink::SinkBuilder;

pub use self::{
    cli::Cli,
    exit_channel::{ExitReceiver, ExitSender, exit_channel},
};

pub(crate) use self::{
    app::App,
    message::{Message, QuickMessage, Safety},
};

mod api_window;
mod app;
mod cli;
mod dialog;
mod exit_channel;
mod message;
mod process_info;
mod run_game;
mod subscription;
mod update;
mod view;
mod init_config;

/// Run application.
pub fn run(
    cli: Cli,
    sink_builder: SinkBuilder,
    exit_recv: Option<ExitReceiver>,
) -> ::color_eyre::Result<()> {
    App::run(cli, sink_builder, exit_recv)
}
