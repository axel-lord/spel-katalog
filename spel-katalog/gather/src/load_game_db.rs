//! Functions to load game database.

use ::std::path::Path;

use ::dashmap::DashMap;
use ::rusqlite::{Connection, OpenFlags};
use ::rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use ::spel_katalog_formats::{Game, GameCommon, GameLutris, Tag, TagId};

use crate::LoadDbError;

/// Attempt to load games from lutris database.
///
/// # Errors
/// If games cannot be loaded from database.
pub fn load_games_from_database(
    db_path: &Path,
    tags: &DashMap<Tag, TagId, FxBuildHasher>,
) -> Result<Vec<Game>, LoadDbError> {
    let db = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let categories = db
        .prepare_cached("SELECT id, name FROM categories")?
        .query_map([], |row| Ok((row.get("id")?, row.get("name")?)))?
        .collect::<Result<FxHashMap<i64, String>, ::rusqlite::Error>>()
        .unwrap_or_default();

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

    let mut stmt =
        db.prepare_cached("SELECT id,name,slug,runner,configpath,installed_at FROM games")?;
    let mut rows = stmt.query([])?;
    let mut games = Vec::new();

    while let Some(row) = rows.next()? {
        fn game_from_row(row: &::rusqlite::Row) -> Option<GameLutris> {
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
            let installed_at = row
                .get("installed_at")
                .map_err(|err| ::log::error!("could not read installed_at of row\n{err}"))
                .ok()?;

            Some(GameLutris {
                slug,
                id,
                runner,
                configpath,
                common: GameCommon {
                    name,
                    installed_at,
                    hidden: false,
                    tags: Default::default(),
                },
            })
        }

        let Some(mut game) = game_from_row(row) else {
            continue;
        };

        if let Some(game_categories) = game_categories.get(&game.id) {
            for category in game_categories {
                let Some(name) = categories.get(category) else {
                    ::log::warn!("unknown lutris category with id: {category}");
                    continue;
                };
                if name == ".hidden" {
                    game.hidden = true;
                    continue;
                }

                let tag = *tags
                    .entry(Tag { name: name.clone() })
                    .or_insert_with(TagId::new);

                game.tags.insert(tag);
            }
        }

        games.push(Game::Lutris(game));
    }

    games.sort_by_key(|game| -game.installed_at);
    Ok(games)
}
