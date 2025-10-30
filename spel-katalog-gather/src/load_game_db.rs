use ::std::path::Path;

use ::rusqlite::{Connection, OpenFlags};
use ::rustc_hash::{FxHashMap, FxHashSet};
use ::spel_katalog_formats::Game;

/// Errors occuring during database load.
#[derive(Debug, ::thiserror::Error)]
pub enum LoadDbError {
    /// A forwarded sqlite error.
    #[error("an sqlite error occurred\n{0}")]
    Sqlite(#[from] ::rusqlite::Error),
}

/// Attempt to load games from lutris database.
pub fn load_games_from_database(db_path: &Path) -> Result<Vec<Game>, LoadDbError> {
    let db = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let categories = db
        .prepare_cached("SELECT id, name FROM categories")?
        .query_map([], |row| Ok((row.get("name")?, row.get("id")?)))?
        .collect::<Result<FxHashMap<String, i64>, ::rusqlite::Error>>();

    let hidden_category = categories
        .as_ref()
        .as_ref()
        .ok()
        .and_then(|categories| categories.get(".hidden").copied())
        .unwrap_or(i64::MAX);

    let game_categories = db
        .prepare_cached("SELECT game_id, category_id FROM games_categories")?
        .query_map([], |row| Ok((row.get("game_id")?, row.get("category_id")?)))?
        .fold(
            FxHashMap::<i64, FxHashSet<i64>>::default(),
            |mut map, result| match result {
                Ok((game, cat)) => {
                    map.entry(game).or_default().insert(cat);
                    map
                }
                Err(err) => {
                    ::log::error!("failed when reading categories\n{err}");
                    map
                }
            },
        );

    let mut stmt = db.prepare_cached("SELECT id,name,slug,runner,configpath FROM games")?;
    let mut rows = stmt.query([])?;
    let mut games = Vec::new();

    while let Some(row) = rows.next()? {
        fn game_from_row(row: &::rusqlite::Row) -> Option<Game> {
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
                batch_selected: false,
            })
        }

        let Some(mut game) = game_from_row(row) else {
            continue;
        };

        if let Some(categories) = game_categories.get(&game.id)
            && categories.contains(&hidden_category)
        {
            game.hidden = true;
        }

        games.push(game);
    }

    games.sort_by_key(|game| -game.id);
    Ok(games)
}
