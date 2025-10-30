//! Utilities for gathering information used by application.

mod load_game_db;

pub use self::load_game_db::{LoadDbError, load_games_from_database};
