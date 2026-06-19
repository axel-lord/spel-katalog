//! Function to initialize config.

use ::std::{io::Write, path::PathBuf};

use ::tap::Pipe;

/// Initialize runtime config directory of application.
pub fn init_config(path: PathBuf) {
    let lib_path = path.join("lib");
    let games_path = path.join("games");
    let batch_path = path.join("batch");
    let scripts_path = path.join("scripts");

    [&path, &lib_path, &games_path, &batch_path, &scripts_path]
        .into_iter()
        .for_each(create_dir_if_missing);

    let config_path = path.join("config.toml");
    create_file_if_missing(&config_path, "");
}

/// Create a directory should it not already exist, and log relevant information.
fn create_dir_if_missing(path: &PathBuf) {
    match ::std::fs::create_dir(path).map_err(|err| (err.kind(), err)) {
        Ok(_) => ::log::info!("created directory {path:?}"),
        Err((::std::io::ErrorKind::AlreadyExists, _)) => {
            ::log::info!("directory {path:?} already exists");
        }
        Err((_, err)) => {
            ::log::error!("could not create directory {path:?}, {err}");
        }
    }
}

/// Create a file with given content should it not already exist, logging relevant information.
fn create_file_if_missing(path: &PathBuf, content: &str) {
    match ::std::fs::File::create_new(path).map_err(|err| (err.kind(), err)) {
        Ok(file) => {
            ::log::info!("created file {path:?}");

            if !content.is_empty() {
                let result = file.pipe(|mut file| {
                    file.write_all(content.as_bytes())?;
                    file.flush()
                });

                if let Err(err) = result {
                    ::log::error!("could not write default content to {path:?}, {err}")
                } else {
                    ::log::info!("wrote default content to {path:?}")
                }
            }
        }
        Err((::std::io::ErrorKind::AlreadyExists, _)) => {
            ::log::info!("file {path:?} already exists");
        }
        Err((_, err)) => {
            ::log::error!("could not create file {path:?}, {err}");
        }
    }
}
