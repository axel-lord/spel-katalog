//! Shared data formats in use buy application.

pub use self::{
    addititional_config::AdditionalConfig,
    bind::{Bind, Symlink},
    game::{Game, GameId},
    image::Image,
    lutris_game::{LutrisGame, LutrisRunner},
    native_game::{NativeGame, NativeRunner},
    timestamp::{TimeStampParseError, Timestamp, TimestampFromIntError},
};

mod addititional_config;
mod bind;
mod game;
mod image;
mod lutris_game;
mod native_game;
mod timestamp;

pub mod lutris_config;
