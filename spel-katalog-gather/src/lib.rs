//! Utilities for gathering information used by application.

mod load_covers;
mod load_game_db;
mod load_thumbnail_db;

pub use self::{
    load_covers::{CoverGatherer, CoverGathererOptions},
    load_game_db::load_games_from_database,
    load_thumbnail_db::load_thumbnail_database,
};

/// Errors occuring during database load.
#[derive(Debug, ::thiserror::Error)]
pub enum LoadDbError {
    /// A forwarded sqlite error.
    #[error("an sqlite error occurred\n{0}")]
    Sqlite(#[from] ::rusqlite::Error),
}
