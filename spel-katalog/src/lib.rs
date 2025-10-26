use ::spel_katalog_cli::Run;
use ::spel_katalog_sink::SinkBuilder;

pub use self::exit_channel::{ExitReceiver, ExitSender, exit_channel};

pub(crate) use self::{
    app::App,
    message::{Message, QuickMessage, Safety},
};

mod api_window;
mod app;
mod dialog;
mod exit_channel;
mod message;
mod process_info;
mod run_game;
mod subscription;
mod update;
mod view;

/// Run application.
pub fn run(
    run: Run,
    sink_builder: SinkBuilder,
    exit_recv: Option<ExitReceiver>,
) -> ::color_eyre::Result<()> {
    App::run(run, sink_builder, exit_recv)
}
