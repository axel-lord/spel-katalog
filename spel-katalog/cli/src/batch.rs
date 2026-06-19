//! [Batch] impl.
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
