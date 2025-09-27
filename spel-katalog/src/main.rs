use ::clap::Parser;
use ::spel_katalog::{App, Cli};

fn main() -> ::color_eyre::Result<()> {
    ::color_eyre::install()?;
    [
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
    .fold(&mut ::env_logger::builder(), |builder, module| {
        builder.filter_module(module, ::log::LevelFilter::Debug)
    })
    .init();
    let cli = Cli::parse();
    App::run(cli)
}
