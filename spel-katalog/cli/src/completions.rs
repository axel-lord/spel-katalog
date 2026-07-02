//! Implementation of completions generator.

use ::std::{io::BufWriter, path::PathBuf};

use ::clap::CommandFactory;

use crate::SubCmdError;

/// Binary to generate completions for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, ::clap::ValueEnum)]
pub enum Binary {
    /// Generate completions for main application.
    #[default]
    SpelKatalog,
    /// Generate completions for game installer application.
    InstallGame,
}

/// Generate command completions.
pub fn completions(
    binary: Binary,
    shell: ::clap_complete::Shell,
    name: Option<String>,
    output: PathBuf,
) -> Result<(), SubCmdError> {
    let (mut command, name) = match binary {
        Binary::SpelKatalog => (
            crate::Cli::command(),
            name.unwrap_or_else(|| "spel-katalog".to_owned()),
        ),
        Binary::InstallGame => (
            ::spel_katalog_install::Cli::command(),
            name.unwrap_or_else(|| "spel-katalog-install".to_owned()),
        ),
    };
    if output.as_os_str().to_str() == Some("-") {
        ::clap_complete::generate(shell, &mut command, name, &mut ::std::io::stdout().lock());
    } else {
        let mut writer = ::std::fs::File::create(&output)
            .map(BufWriter::new)
            .map_err(SubCmdError::open_create(&output))?;
        ::clap_complete::generate(shell, &mut command, name, &mut writer);
    }
    Ok(())
}
