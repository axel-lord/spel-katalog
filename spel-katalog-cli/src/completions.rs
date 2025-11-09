//! Implementation of completions generator.

use ::std::{io::BufWriter, path::PathBuf};

use ::clap::CommandFactory;

use crate::{Cli, SubCmdError};

/// Generate command completions.
pub fn completions(
    shell: ::clap_complete::Shell,
    name: String,
    output: PathBuf,
) -> Result<(), SubCmdError> {
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
    Ok(())
}
