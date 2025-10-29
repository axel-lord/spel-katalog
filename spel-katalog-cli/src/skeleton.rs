use ::std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use crate::SubCmdError;

pub fn skeleton(
    output: PathBuf,
    settings: ::spel_katalog_settings::Settings,
) -> Result<(), SubCmdError> {
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
            ::toml::to_string_pretty(&settings.skeleton()).map_err(SubCmdError::SkeletonToToml)?,
        ),
        writer,
    )
    .map_err(SubCmdError::write_skeleton(&output))?;
    writer.flush().map_err(SubCmdError::close_flush(&output))?;
    Ok(())
}
