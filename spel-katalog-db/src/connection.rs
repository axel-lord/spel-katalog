//! [ManagerConnection] impl.

use ::rusqlite::Connection;

use crate::{DbPath, PerformError};

/// Connection to database manager.
#[derive(Debug, Clone)]
pub struct ManagerConnection {
    /// Sender used to contact manager.
    pub(crate) sender: ::flume::Sender<Message>,
}

impl ManagerConnection {
    /// Perform an action on a database connection.
    ///
    /// # Errors
    /// If the database manager has stopped.
    pub fn perform<D, F>(&self, db: D, f: F) -> Result<(), PerformError>
    where
        D: Into<DbPath>,
        F: FnOnce(&Connection) + 'static + Send,
    {
        self.sender
            .send(Message {
                db: db.into(),
                f: Box::new(f),
            })
            .map_err(|err| PerformError {
                db: err.into_inner().db,
            })
    }
}

/// Message sent to database manager.
pub(crate) struct Message {
    /// Database name.
    pub db: DbPath,
    /// Action to perform.
    pub f: Box<dyn FnOnce(&Connection) + Send>,
}
