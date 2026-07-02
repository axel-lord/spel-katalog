//! SubCmd impl.

use ::std::path::{Path, PathBuf};

use ::clap::Subcommand;
use ::spel_katalog_install::InstallGame;

use crate::{
    completions::{Binary, completions},
    init_config::init_config,
    skeleton::skeleton,
};

/// Get default shell.
fn get_shell() -> ::clap_complete::Shell {
    ::clap_complete::Shell::from_env().unwrap_or(::clap_complete::Shell::Bash)
}

/// Callbacks required when performing subcommand.
#[derive(Debug)]
pub struct SubcmdCallbacks {
    /// Callback to use when running application.
    pub run: fn(crate::Run) -> ::color_eyre::Result<()>,
}

/// Error returned whe subcmd perform fails.
#[derive(Debug, ::thiserror::Error)]
pub enum SubCmdError {
    /// A file could not be crated and/or opened.
    #[error("could not create/open {path:?}\n{source}")]
    OpenCreate {
        /// Io error as source.
        source: ::std::io::Error,
        /// Path to file.
        path: PathBuf,
    },

    /// A File could not be flushed and/or closed.
    #[error("could not close/flush {path:?}\n{source}")]
    CloseFlush {
        /// Io error as source.
        source: ::std::io::Error,
        /// Path to file.
        path: PathBuf,
    },

    /// Skeleton could not be written.
    #[error("could write skeleton to {path:?}\n{source}")]
    WriteSkeleton {
        /// Io error as source.
        source: ::std::io::Error,
        /// Path to file.
        path: PathBuf,
    },

    /// Skeleton could not be converted to toml.
    #[error("could not convert skeleton to toml\n{0}")]
    SkeletonToToml(#[source] ::toml::ser::Error),
}

impl SubCmdError {
    /// Create a closure mapping io errors to self using provided path.
    pub(crate) fn close_flush(path: &Path) -> impl FnOnce(::std::io::Error) -> Self {
        |source| Self::CloseFlush {
            source,
            path: path.to_path_buf(),
        }
    }

    /// Create a closure mapping io errors to self using provided path.
    pub(crate) fn open_create(path: &Path) -> impl FnOnce(::std::io::Error) -> Self {
        |source| Self::OpenCreate {
            source,
            path: path.to_path_buf(),
        }
    }
    /// Create a closure mapping io errors to self using provided path.
    pub(crate) fn write_skeleton(path: &Path) -> impl FnOnce(::std::io::Error) -> Self {
        |source| Self::WriteSkeleton {
            source,
            path: path.to_path_buf(),
        }
    }
}

/// Use cases other than launching gui.
#[derive(Debug, Subcommand)]
pub enum Subcmd {
    /// Run application.
    Run(#[command(flatten)] crate::Run),
    /// Output a skeleton config.
    Skeleton {
        /// Settings to set for skeleton.
        #[command(flatten)]
        settings: ::spel_katalog_settings::SettingsArgs,

        /// Where to write skeleton to.
        #[arg(long, short, default_value = "-")]
        output: PathBuf,
    },
    /// Output completions.
    Completions {
        /// Shell to generate completions for.
        #[arg(short, long, value_enum, default_value_t = get_shell())]
        shell: ::clap_complete::Shell,
        /// Name of the binary completions should be generated for.
        #[arg(short, long)]
        name: Option<String>,
        /// Where to write completions to.
        #[arg(short, long, default_value = "-")]
        output: PathBuf,
        /// Which application to generate completions for.
        #[arg(short, long, value_enum, default_value_t)]
        binary: Binary,
    },
    /// Generate missing config. And/Or update lua definition.
    InitConfig {
        /// Path to config directory.
        path: PathBuf,
    },
    /// Open game installer.
    InstallGame(#[command(flatten)] InstallGame),
}

impl Default for Subcmd {
    fn default() -> Self {
        Self::Run(crate::Run::default())
    }
}

impl Subcmd {
    /// Perform action tied to subcommand.
    ///
    /// # Errors
    /// Forwards whatever errors may occur in callback for given subcommand.
    pub fn perform(self, callbacks: SubcmdCallbacks) -> ::color_eyre::Result<()> {
        let SubcmdCallbacks { run } = callbacks;
        match self {
            Subcmd::Skeleton { output, settings } => {
                skeleton(output, settings.into())?;
            }
            Subcmd::Completions {
                shell,
                name,
                output,
                binary,
            } => {
                completions(binary, shell, name, output)?;
            }
            Subcmd::InitConfig { path } => {
                init_config(path);
            }
            Subcmd::Run(cli) => run(cli)?,
            Subcmd::InstallGame(game) => {
                game.run()?;
            }
        }
        Ok(())
    }
}
