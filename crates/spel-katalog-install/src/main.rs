//! Dedicated install-game binary.

use ::clap::Parser;
use ::color_eyre::Result;
use ::log::LevelFilter;
use ::spel_katalog_install::Cli;

fn main() -> Result<()> {
    ::color_eyre::install()?;
    ::env_logger::builder()
        .filter_level(LevelFilter::Info)
        .init();
    let Cli { args } = Parser::parse();
    args.run()
}
