use ::std::io::{IsTerminal, Read};

use ::mimalloc::MiMalloc;
use ::spel_katalog::run as run_app;
use ::spel_katalog_cli::{Cli, Subcmd, SubcmdCallbacks};
use ::spel_katalog_sink::SinkBuilder;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn run(cli: ::spel_katalog_cli::Run) -> ::color_eyre::Result<()> {
    let keep_terminal = cli.keep_terminal;
    run_app(cli, SinkBuilder::Inherit, None)?;

    if keep_terminal && ::std::io::stdin().is_terminal() {
        println!("Press enter to exit...");
        let mut buf = [0u8; 1];
        ::std::io::stdin().read_exact(&mut buf)?;
    }

    Ok(())
}

fn main() -> ::color_eyre::Result<()> {
    ::color_eyre::install()?;
    let cli = Cli::parse();
    let cmd = Subcmd::from(cli);
    let mut log_builder = ::env_logger::builder();
    log_builder.filter_level(::log::LevelFilter::Info);

    if let Some(target) = None {
        log_builder.target(target).init();
    } else {
        log_builder.init();
    }
    cmd.perform(SubcmdCallbacks { run })
}
