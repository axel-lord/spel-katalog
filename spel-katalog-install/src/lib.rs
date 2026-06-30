//! Game installation cli to ipc message.

use ::std::collections::HashMap;
use ::std::path::PathBuf;

use ::clap::{Args, Parser};
use ::color_eyre::{Result, Section, eyre::eyre};
use ::spel_katalog_formats::{Bind, InstallerConfig};

/// Install a game.
#[derive(Debug, Parser)]
pub struct Cli {
    /// Install args.
    #[command(flatten)]
    pub args: InstallGame,
}

/// Install a game.
#[derive(Debug, Args)]
pub struct InstallGame {
    /// Game to install.
    pub game: PathBuf,
    /// Thumbnail of game.
    pub thumbnail: Option<PathBuf>,
    /// Should the game be hidden.
    #[arg(long)]
    pub hidden: bool,
    /// Should the game not be moved.
    #[arg(long)]
    pub no_move: bool,
    /// Exe to use for game.
    #[arg(long)]
    pub exe: Option<PathBuf>,
    /// Add a directory with an installer.
    #[arg(long)]
    pub installer_dir: Option<PathBuf>,
    /// Set exe to a dummy value.
    #[arg(long, conflicts_with = "exe")]
    pub dummy: bool,
}

impl InstallGame {
    /// Run install-game.
    ///
    /// # Errors
    /// If game installer cannot be ran.
    pub fn run(self) -> Result<()> {
        let InstallGame {
            game,
            thumbnail,
            hidden,
            no_move,
            exe,
            installer_dir,
            dummy,
        } = self;
        let base_dirs = ::xdg::BaseDirectories::with_prefix("spel-katalog");
        let drives = installer_dir
            .as_ref()
            .map(|d| HashMap::from_iter([('i', d.clone())]))
            .unwrap_or_default();
        let ro_bind = installer_dir
            .map(|d| Vec::from_iter([Bind::mirrored(d)]))
            .unwrap_or_default();
        let game_dir = game
            .canonicalize()
            .map_err(|err| eyre!(err).note(format!("is {game:?} a valid path?")))?;
        let exe = if !dummy {
            exe
        } else {
            Some(game_dir.join("dummy.exe"))
        };

        if let Err(err) = ::spel_katalog_ipc::send(
            &base_dirs,
            ::spel_katalog_ipc::Message::InstallGame(InstallerConfig {
                game_dir,
                exe,
                hidden: Some(hidden),
                thumbnail: thumbnail
                    .map(|t| {
                        t.canonicalize()
                            .map_err(|err| eyre!(err).note(format!("is {t:?} a valid path?")))
                    })
                    .transpose()?,
                move_game: Some(!no_move),
                drives,
                bind: Default::default(),
                ro_bind,
                env: Default::default(),
            }),
        ) {
            Err(eyre!(err).note("is the application open?"))
        } else {
            Ok(())
        }
    }
}
