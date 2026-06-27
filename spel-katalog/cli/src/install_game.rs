//! Game install cli.

use ::std::path::PathBuf;

/// Install a game.
#[derive(Debug, ::clap::Args)]
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
}
