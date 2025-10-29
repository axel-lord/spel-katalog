//! Cli command to run application.
use ::std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use ::clap::Args;

pub fn default_config() -> &'static Path {
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
#[derive(Debug, Args)]
pub struct Run {
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

    /// Show a terminal dialog.
    #[arg(long, visible_alias = "st", conflicts_with = "advanced_terminal")]
    pub show_terminal: bool,

    /// At most how long to wait for application initialization to finish before running batch.
    #[arg(long, requires = "batch", required = false, default_value_t = 120)]
    pub batch_init_timeout: u16,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            settings: ::spel_katalog_settings::Settings::default(),
            show_settings: false,
            config: default_config().to_path_buf(),
            advanced_terminal: false,
            keep_terminal: false,
            batch: None,
            batch_init_timeout: 120,
            show_terminal: false,
        }
    }
}
