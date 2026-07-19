use ::std::path::Path;

use ::spel_katalog_cli::Run;
use ::spel_katalog_sink::SinkBuilder;

pub use self::exit_channel::{ExitReceiver, ExitSender, exit_channel};

pub(crate) use self::{
    app::App,
    message::{Message, QuickMessage, Safety},
};

mod app;
mod exit_channel;
mod message;
mod process_info;
mod run_game;
mod subscription;
mod update;
mod view;

pub mod oneshot_broadcast;

/// Element alias
type Element<'a, M> = ::iced_core::Element<'a, M, ::iced_core::Theme, ::iced_renderer::Renderer>;

/// Get settings.
pub fn get_settings(
    config: &Path,
    overrides: ::spel_katalog_settings::SettingsArgs,
) -> ::spel_katalog_settings::Settings {
    ::spel_katalog_settings::load(config, overrides)
}

/// Run application.
pub fn run(
    run: Run,
    sink_builder: SinkBuilder,
    exit_recv: Option<ExitReceiver>,
) -> ::color_eyre::Result<()> {
    App::run(run, sink_builder, exit_recv)
}
