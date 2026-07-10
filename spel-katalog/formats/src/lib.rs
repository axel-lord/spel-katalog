//! Shared data formats in use buy application.

pub use self::{
    addititional_config::AdditionalConfig,
    bind::{Bind, Symlink},
    daemon::{DaemonRunConfigRequest, DaemonRunResponse},
    game::{
        Game, GameId,
        common::GameCommon,
        lutris::{GameLutris, RunnerLutris},
        native::GameNative,
    },
    image::Image,
    installer::{ExeChoice, InstallerConfig, InstallerPrepareConfig},
    native_game_config::{NativeGameConfig, RunMode, RunnerNative},
    tag::{Tag, TagId},
    timestamp::{TimeStampParseError, Timestamp, TimestampFromIntError},
};

mod addititional_config;
mod bind;
mod daemon;
mod game;
mod image;
mod installer;
mod native_game_config;
mod tag;
mod timestamp;

pub mod lutris_config;
