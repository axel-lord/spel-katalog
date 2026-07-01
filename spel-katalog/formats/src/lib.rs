//! Shared data formats in use buy application.

pub use self::{
    addititional_config::AdditionalConfig,
    bind::{Bind, Symlink},
    daemon::{DaemonRunConfigRequest, DaemonRunResponse},
    game::{Game, GameId},
    image::Image,
    installer::{ExeChoice, InstallerConfig, InstallerPrepareConfig},
    lutris_game::{LutrisGame, LutrisRunner},
    native_game::{NativeGame, NativeRunner, RunMode},
    timestamp::{TimeStampParseError, Timestamp, TimestampFromIntError},
};

mod addititional_config;
mod bind;
mod daemon;
mod game;
mod image;
mod installer;
mod lutris_game;
mod native_game;
mod timestamp;

pub mod lutris_config;
