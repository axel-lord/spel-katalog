//! Dedicated install-game binary.

use ::clap::Parser;
use ::color_eyre::Result;
use ::log::LevelFilter;
use ::spel_katalog_install::{InstallGame, install_game};

/// Install a game.
#[derive(Debug, Parser)]
struct Cli {
    /// Install args.
    #[command(flatten)]
    args: InstallGame,
}

fn main() -> Result<()> {
    ::color_eyre::install()?;
    ::env_logger::builder()
        .filter_level(LevelFilter::Info)
        .init();
    let Cli { args } = Parser::parse();
    install_game(args)
}
