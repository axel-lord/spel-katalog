use ::std::{
    io::{IsTerminal, Read},
    path::Path,
};

use ::color_eyre::{Section, eyre::eyre};
use ::mimalloc::MiMalloc;
use ::spel_katalog::run as run_app;
use ::spel_katalog_cli::{Cli, InstallGame, Subcmd, SubcmdCallbacks};
use ::spel_katalog_sink::SinkBuilder;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn init_log(target: Option<::env_logger::Target>) {
    let mut log_builder = ::env_logger::builder();
    log_builder.filter_level(::log::LevelFilter::Info);

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

fn install_game(
    InstallGame {
        game,
        thumbnail,
        hidden,
        no_move,
        exe,
    }: InstallGame,
) -> ::color_eyre::Result<()> {
    init_log(None);
    let base_dirs = ::xdg::BaseDirectories::with_prefix("spel-katalog");
    if let Err(err) = ::spel_katalog_ipc::send(
        base_dirs
            .get_runtime_directory()
            .map(|dir| dir.as_path())
            .unwrap_or_else(|err| {
                ::log::error!("could not get runtime directory, using /tmp\n{err}");
                Path::new("/tmp")
            }),
        ::spel_katalog_ipc::Message::InstallGame {
            source: game
                .canonicalize()
                .map_err(|err| eyre!(err).note(format!("is {game:?} a valid path?")))?,
            hidden,
            move_game: !no_move,
            thumbnail: thumbnail
                .map(|t| {
                    t.canonicalize()
                        .map_err(|err| eyre!(err).note(format!("is {t:?} a valid path?")))
                })
                .transpose()?,
            exe,
        },
    ) {
        Err(eyre!(err).note("is the application open?"))
    } else {
        Ok(())
    }
}

fn main() -> ::color_eyre::Result<()> {
    ::color_eyre::install()?;
    let cli = Cli::parse();
    let cmd = Subcmd::from(cli);
    cmd.perform(SubcmdCallbacks {
        run,
        other,
        install_game,
    })
}
