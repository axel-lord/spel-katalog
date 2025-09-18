use ::iced::widget::image::Handle;
use ::rusqlite::Row;

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
    /// Is the game hidden.
    pub hidden: bool,
    /// Thumbnail in use.
    pub image: Option<Handle>,
    /// Is the game selected for batch commands.
    pub batch_selected: bool,
}

impl Game {
    /// Read a game from a database row.
    pub fn from_row(row: &Row) -> Option<Self> {
        let slug = row
            .get("slug")
            .map_err(|err| ::log::error!("could not read slug of row\n{err}"))
            .ok()?;
        let id = row
            .get("id")
            .map_err(|err| ::log::error!("could not read id of row\n{err}"))
            .ok()?;
        let name = row
            .get("name")
            .map_err(|err| ::log::error!("could not read name of row\n{err}"))
            .ok()?;
        let runner = row
            .get_ref("runner")
            .map_err(|err| ::log::error!("could not read runner of row\n{err}"))
            .ok()?
            .as_str()
            .map_err(|err| ::log::error!("could not get runner of row as a string\n{err}"))
            .ok()?
            .into();
        let configpath = row
            .get("configpath")
            .map_err(|err| ::log::error!("could not read configpath of row\n{err}"))
            .ok()?;

        Some(Game {
            slug,
            id,
            name,
            runner,
            configpath,
            hidden: false,
            image: None,
            batch_selected: false,
        })
    }
}
