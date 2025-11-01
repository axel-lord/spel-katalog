//! Cli spec for project.

use ::clap::Parser;

pub use self::{
    batch::Batch,
    run::Run,
    subcmd::{SubCmdError, Subcmd, SubcmdCallbacks},
};

mod completions;
mod init_config;
mod run;
mod skeleton;
mod subcmd;
mod batch {
    use ::std::path::PathBuf;

    use ::clap::Args;

    use crate::run::default_config;

    /// Run batch scripts without starting full application.
    #[derive(Debug, Args)]
    pub struct Batch {
        /// Batch scripts to run
        #[arg(required = true)]
        pub script: Vec<PathBuf>,

        /// Settings to set for batch run.
        #[command(flatten)]
        pub settings: ::spel_katalog_settings::Settings,

        /// Config file to load.
        #[arg(long, short, default_value=default_config().as_os_str())]
        pub config: PathBuf,
    }
}

/// Application cli.
#[derive(Debug, Parser)]
#[command(author, version, long_about = None)]
pub struct Cli {
    /// Action to perform.
    #[command(subcommand)]
    pub action: Option<Subcmd>,
}

impl Cli {
    /// Parse Cli.
    pub fn parse() -> Self {
        Parser::parse()
    }
}

impl From<Cli> for Subcmd {
    fn from(value: Cli) -> Self {
        value.action.unwrap_or_default()
    }
}
