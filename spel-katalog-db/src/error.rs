//! [PerformError] impl.

use crate::DbPath;

/// Error for when a database action cannot be performed.
#[derive(Debug, ::thiserror::Error)]
#[error("could not send action to {:?}", .db)]
pub struct PerformError {
    /// Path of database.
    pub(crate) db: DbPath,
}
