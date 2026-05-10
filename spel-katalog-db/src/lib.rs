//! Higher level database access.

use ::core::time::Duration;
use ::std::{collections::hash_map::Entry, panic::catch_unwind, time::Instant};

use crate::connection::Message;

pub use self::{connection::ManagerConnection, db_path::DbPath, error::PerformError};
use ::flume::{Receiver, RecvTimeoutError, unbounded};
use ::rusqlite::Connection;
use ::rustc_hash::FxHashMap;

mod connection;
mod db_path;
mod error;

/// Timeout before cleaning is attempted.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Start database manager.
///
/// # Errors
/// If the database manager thread cannot be started.
pub fn start() -> Result<ManagerConnection, ::std::io::Error> {
    let (sender, rx) = unbounded();
    let _handle = ::std::thread::Builder::new()
        .name("spel-katalog-database-manager".to_owned())
        .spawn(move || {
            loop {
                match catch_unwind(|| start_(&rx)) {
                    Ok(()) => {
                        ::log::info!("database manager closed due to disconnect");
                        break;
                    }
                    Err(_) => {
                        ::log::error!("database manager restarted due to panic");
                        continue;
                    }
                }
            }
        })?;

    Ok(ManagerConnection { sender })
}

/// Close given connections.
fn close_connections(
    connections: &mut dyn Iterator<Item = (DbPath, (Instant, Connection))>,
) -> Vec<(DbPath, (Instant, Connection))> {
    let mut failures = Vec::new();
    for (db_path, (last_act, conn)) in connections {
        if let Err((conn, err)) = conn.close() {
            ::log::warn!(
                "failed to close connection to {db_path:?}\n{err}",
                db_path = db_path.as_path()
            );
            failures.push((db_path, (last_act, conn)));
        } else {
            ::log::info!("closed database connection to {:?}", db_path.as_path())
        }
    }
    failures
}

/// Database thread function.
fn start_(rx: &Receiver<Message>) {
    let mut db_catalog = FxHashMap::<DbPath, (Instant, Connection)>::default();
    let mut last_cleanup = Instant::now();

    loop {
        let Message { db, f } = match rx.recv_deadline(last_cleanup + TIMEOUT) {
            Ok(msg) => msg,
            Err(RecvTimeoutError::Timeout) => {
                let now = Instant::now();
                let failures = close_connections(
                    &mut db_catalog
                        .extract_if(|_, (last_act, _)| now.duration_since(*last_act) > TIMEOUT),
                );
                db_catalog.extend(failures);
                last_cleanup = Instant::now();
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => {
                return;
            }
        };

        let mut conn = match db_catalog.entry(db) {
            Entry::Occupied(occupied_entry) => occupied_entry,
            Entry::Vacant(vacant_entry) => {
                let db_path = vacant_entry.key().as_path();
                let conn = match ::rusqlite::Connection::open(db_path) {
                    Ok(conn) => conn,
                    Err(err) => {
                        ::log::error!("failed to open sqlite database {db_path:?}\n{err}");
                        continue;
                    }
                };

                vacant_entry.insert_entry((Instant::now(), conn))
            }
        };

        let (last_act, conn) = conn.get_mut();
        f(conn);
        *last_act = Instant::now();
    }
}
