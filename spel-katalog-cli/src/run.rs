//! Cli command to run application.
use ::std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use ::clap::Args;

/// Generate default config path.
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
    #[arg(long, visible_alias = "kt")]
    pub keep_terminal: bool,

    /// Show a terminal dialog.
    #[arg(long, visible_alias = "st", conflicts_with = "advanced_terminal")]
    pub show_terminal: bool,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            settings: ::spel_katalog_settings::Settings::default(),
            show_settings: false,
            config: default_config().to_path_buf(),
            advanced_terminal: false,
            keep_terminal: false,
            show_terminal: false,
        }
    }
}
