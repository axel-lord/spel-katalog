//! Implementation of children request.

use ::std::path::Path;

use ::color_eyre::eyre::Context;
use ::smol::{fs, stream::StreamExt};
use ::spel_katalog_formats::daemon;
use ::spel_katalog_ipc::http::HttpResponse;

/// Get child processes.
///
/// # Errors
/// If the processes cannot be collected.
pub async fn children() -> Result<daemon::response::Children, HttpResponse> {
    let task_dir = Path::new("/proc/self/task/");
    let read_dir = fs::read_dir(task_dir)
        .await
        .wrap_err_with(|| format!("could not read directory {task_dir:?}"))?;

    let mut response = daemon::response::Children::default();

    read_dir
        .filter_map(|entry| entry.ok())
        .then(async |entry| {
            let path = entry.path().join("children");
            fs::read_to_string(&path)
                .await
                .map_err(|err| ::log::error!("could not read {path:?}\n{err}"))
                .ok()
        })
        .filter_map(::core::convert::identity)
        .for_each(|task_children| {
            for line in task_children.split_ascii_whitespace() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let Ok(pid) = line.parse::<i64>() else {
                    continue;
                };

                response.push(pid);
            }
        })
        .await;

    Ok(response)
}
