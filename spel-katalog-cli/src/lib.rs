//! Cli spec for project.

use ::clap::Parser;

pub use self::{
    run::Run,
    subcmd::{SubCmdError, Subcmd, SubcmdCallbacks},
};

mod completions;
mod init_config;
mod run;
mod skeleton;
mod subcmd;

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
