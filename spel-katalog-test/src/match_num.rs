//! Sample game, should run on all platforms, and log with color.

use ::clap::Parser;
use ::log::LevelFilter;

#[derive(Debug, Parser)]
#[command(author, version, long_about = None)]
struct Cli {}

struct State {
    cells: Vec<Vec<()>>,
}

/// Application entry.
fn main() -> ::color_eyre::Result<()> {
    let Cli {} = Cli::parse();
    ::color_eyre::install()?;
    ::env_logger::builder()
        .filter_level(LevelFilter::max())
        .write_style(::env_logger::WriteStyle::Always)
        .init();
    Ok(())
}
