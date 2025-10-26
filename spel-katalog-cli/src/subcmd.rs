//! SubCmd impl.

use ::std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use ::clap::{CommandFactory, Subcommand};

use crate::{Cli, init_config::init_config};

fn get_shell() -> ::clap_complete::Shell {
    ::clap_complete::Shell::from_env().unwrap_or_else(|| ::clap_complete::Shell::Bash)
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
    fn close_flush(path: &Path) -> impl FnOnce(::std::io::Error) -> Self {
        |source| Self::CloseFlush {
            source,
            path: path.to_path_buf(),
        }
    }

    /// Create a closure mapping io errors to self using provided path.
    fn open_create(path: &Path) -> impl FnOnce(::std::io::Error) -> Self {
        |source| Self::OpenCreate {
            source,
            path: path.to_path_buf(),
        }
    }
    /// Create a closure mapping io errors to self using provided path.
    fn write_skeleton(path: &Path) -> impl FnOnce(::std::io::Error) -> Self {
        |source| Self::WriteSkeleton {
            source,
            path: path.to_path_buf(),
        }
    }
}

/// Use cases other than launching gui.
#[derive(Debug, Subcommand)]
pub enum Subcmd {
    /// Output a skeleton config.
    Skeleton {
        /// Where to write skeleton to.
        #[arg(long, short, default_value = "-")]
        output: PathBuf,
    },
    /// Output completions.
    Completions {
        /// Shell to use.
        #[arg(short, long, value_enum, default_value_t = get_shell())]
        shell: ::clap_complete::Shell,
        /// Name of the binary completions should be generated for.
        #[arg(short, long, default_value = "spel-katalog")]
        name: String,
        /// Where to write completions to.
        #[arg(short, long, default_value = "-")]
        output: PathBuf,
    },
    /// Generate missing config. And/Or update lua definition.
    InitConfig {
        /// Path to config directory.
        path: PathBuf,
        /// Do not update lua definition.
        #[arg(long)]
        skip_lua_update: bool,
    },
}

impl Subcmd {
    /// Perform action tied to subcommand.
    pub fn perform(self, settings: &::spel_katalog_settings::Settings) -> Result<(), SubCmdError> {
        match self {
            Subcmd::Skeleton { output } => {
                let mut stdout;
                let mut file;
                let writer: &mut dyn Write;
                if output.as_os_str().to_str() == Some("-") {
                    stdout = ::std::io::stdout().lock();
                    writer = &mut stdout;
                } else {
                    file = ::std::fs::File::create(&output)
                        .map(BufWriter::new)
                        .map_err(SubCmdError::open_create(&output))?;
                    writer = &mut file;
                }
                ::std::io::copy(
                    &mut ::std::io::Cursor::new(
                        ::toml::to_string_pretty(&settings.skeleton())
                            .map_err(SubCmdError::SkeletonToToml)?,
                    ),
                    writer,
                )
                .map_err(SubCmdError::write_skeleton(&output))?;
                writer.flush().map_err(SubCmdError::close_flush(&output))?;
            }
            Subcmd::Completions {
                shell,
                name,
                output,
            } => {
                if output.as_os_str().to_str() == Some("-") {
                    ::clap_complete::generate(
                        shell,
                        &mut Cli::command(),
                        name,
                        &mut ::std::io::stdout().lock(),
                    );
                } else {
                    let mut writer = ::std::fs::File::create(&output)
                        .map(BufWriter::new)
                        .map_err(SubCmdError::open_create(&output))?;
                    ::clap_complete::generate(shell, &mut Cli::command(), name, &mut writer);
                }
            }
            Subcmd::InitConfig {
                path,
                skip_lua_update,
            } => {
                init_config(path, skip_lua_update);
            }
        }
        Ok(())
    }
}
