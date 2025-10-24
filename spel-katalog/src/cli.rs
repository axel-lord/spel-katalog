use ::std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use ::clap::{Parser, Subcommand};

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

fn get_shell() -> ::clap_complete::Shell {
    ::clap_complete::Shell::from_env().unwrap_or_else(|| ::clap_complete::Shell::Bash)
}

#[derive(Debug, Parser)]
#[command(author, version)]
pub struct Cli {
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
    #[arg(long, visible_alias = "kt", requires("advanced_terminal"))]
    pub keep_terminal: bool,

    /// Perform an action other than opening gui.
    #[command(subcommand)]
    pub action: Option<Subcmd>,
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
