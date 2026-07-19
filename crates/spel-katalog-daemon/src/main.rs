//! Application daemon.

use ::clap::Parser;
use ::log::LevelFilter;
use ::spel_katalog_daemon::Cli;

fn main() -> ::color_eyre::Result<()> {
    ::color_eyre::install()?;
    ::env_logger::builder()
        .filter_level(LevelFilter::Info)
        .init();

    let Cli { args } = Parser::parse();

    args.run()
}
