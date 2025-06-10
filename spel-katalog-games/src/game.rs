use ::iced::widget::image::Handle;
use ::sqlite::Row;

use crate::Runner;

/// The stored configuration for a game.
#[derive(Debug, Clone)]
pub struct Game {
    /// Slug assinged in lutris.
    pub slug: String,
    /// Numeric id of game.
    pub id: i64,
    /// Title used for game.
    pub name: String,
    /// Runner in use.
    pub runner: Runner,
    /// Path to lutris yml for game.
    pub configpath: String,
    /// Thumbnail in use.
    pub image: Option<Handle>,
}

impl Game {
    /// Read a game from a database row.
    pub fn from_row(row: &Row) -> Option<Self> {
        let slug = row
            .try_read::<&str, _>("slug")
            .map_err(|err| ::log::error!("could not read slug of row\n{err}"))
            .ok()?
            .into();
        let id = row
            .try_read::<i64, _>("id")
            .map_err(|err| ::log::error!("could not read id of row\n{err}"))
            .ok()?
            .into();
        let name = row
            .try_read::<&str, _>("name")
            .map_err(|err| ::log::error!("could not read name of row\n{err}"))
            .ok()?
            .into();
        let runner = row
            .try_read::<&str, _>("runner")
            .map_err(|err| ::log::error!("could not read runner of row\n{err}"))
            .ok()?
            .into();
        let configpath = row
            .try_read::<&str, _>("configpath")
            .map_err(|err| ::log::error!("could not read configpath of row\n{err}"))
            .ok()?
            .into();

        Some(Game {
            slug,
            id,
            name,
            runner,
            configpath,
            image: None,
        })
    }
}
