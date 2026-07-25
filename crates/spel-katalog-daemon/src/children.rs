//! Implementation of children request.

use ::std::path::Path;

use ::bytes::Bytes;
use ::color_eyre::eyre::Context;
use ::smol::{fs, stream::StreamExt};
use ::spel_katalog_formats::daemon;
use ::spel_katalog_ipc::http::HttpResponse;

/// Get child processes.
///
/// # Errors
/// If the processes cannot be collected.
pub async fn children() -> Result<Bytes, HttpResponse> {
    let task_dir = Path::new("/proc/self/task/");
    let mut read_dir = fs::read_dir(task_dir)
        .await
        .wrap_err_with(|| format!("could not read directory {task_dir:?}"))?;

    let mut response = daemon::response::Children::default();
    while let Some(entry) = read_dir.next().await {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                ::log::error!("encountered error reading {task_dir:?}\n{err}");
                continue;
            }
        };

        let file_name = entry.file_name();
        let file_name = match file_name.to_str() {
            Some(file_name) => file_name,
            None => {
                ::log::error!("could not convert file name {file_name:?} to utf-8");
                continue;
            }
        };

        let pid = match file_name.parse::<i64>() {
            Ok(pid) => pid,
            Err(err) => {
                ::log::error!("could not parse {file_name} as a 64 bit signed integer\n{err}");
                continue;
            }
        };

        response.push(pid);
    }

    let response = ::serde_json::to_vec(&response)
        .wrap_err("could not serialize response for children request")?;

    Ok(Bytes::from_owner(response))
}
