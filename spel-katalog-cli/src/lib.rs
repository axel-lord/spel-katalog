//! Cli spec for project.

use ::std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use ::clap::Parser;

pub use self::subcmd::{SubCmdError, Subcmd};

mod init_config;
mod subcmd;

fn default_config() -> &'static Path {
    static LAZY: OnceLock<PathBuf> = OnceLock::new();
    LAZY.get_or_init(|| {
        let mut cfg = PathBuf::from(::spel_katalog_settings::HOME.as_str());
        cfg.push(".config");
        cfg.push("spel-katalog");
        cfg.push("config.toml");
        cfg
    })
}

/// Application Cli.
#[derive(Debug, Parser)]
#[command(author, version, args_conflicts_with_subcommands = true)]
pub struct Cli {
    /// Settings to set for this run.
    #[command(flatten)]
    pub settings: ::spel_katalog_settings::Settings,

    /// Show settings at startup.
    #[arg(long)]
    pub show_settings: bool,

    /// Config file to load.
    #[arg(long, short, default_value=default_config().as_os_str())]
    pub config: PathBuf,

    /// Advanced terminal output.
    #[arg(long, visible_alias = "at")]
    pub advanced_terminal: bool,

    /// Keep terminal open.
    #[arg(long, visible_alias = "kt", requires = "advanced_terminal")]
    pub keep_terminal: bool,

    /// Run the given batch script, then exit.
    #[arg(long, short, conflicts_with = "advanced_terminal")]
    pub batch: Option<PathBuf>,

    /// At most how long to wait for application initialization to finish before running batch.
    #[arg(long, requires = "batch", required = false, default_value_t = 120)]
    pub batch_init_timeout: u16,

    /// Perform an action other than opening gui.
    #[command(subcommand)]
    pub action: Option<Subcmd>,
}

impl Cli {
    /// Parse Cli.
    pub fn parse() -> Self {
        Parser::parse()
    }
}
