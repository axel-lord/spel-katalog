//! [Games] impl.

use ::std::{cell::OnceCell, iter};

use ::iced::widget::image::Handle;
use ::rustc_hash::FxHashMap;
use ::spel_katalog_settings::Settings;

use crate::Game;

#[derive(Debug)]
struct GameCache {
    slug: String,
    name: String,
}

/// Collection of games.
#[derive(Debug, Default)]
pub struct Games {
    cache: Box<[OnceCell<GameCache>]>,
    games: Box<[Game]>,
    displayed: Vec<usize>,
    slug_lookup: FxHashMap<String, usize>,
    id_lookup: FxHashMap<i64, usize>,
}

impl Games {
    /// Games that are to currently be dfisplayed.
    pub fn displayed(&self) -> impl Iterator<Item = &'_ Game> {
        self.displayed.iter().filter_map(|idx| self.games.get(*idx))
    }

    /// All games.
    pub fn all(&self) -> &[Game] {
        &self.games
    }

    /// Amount of games.
    pub fn all_count(&self) -> usize {
        self.games.len()
    }

    /// Amount of displayed games.
    pub fn displayed_count(&self) -> usize {
        self.displayed.len()
    }

    /// Sort displayed games.
    pub fn sort(&mut self, settings: &Settings, filter: &str) {
        let Self {
            games,
            displayed,
            slug_lookup: _,
            id_lookup: _,
            cache,
        } = self;

        let to_be = 0..games.len();

        let get_game_cache = |idx: usize| -> Option<(&Game, &GameCache)> {
            let game = games.get(idx)?;
            let cache = cache.get(idx)?.get_or_init(|| GameCache {
                slug: game.slug.to_uppercase(),
                name: game.name.to_uppercase(),
            });
            Some((game, cache))
        };

        let sort_items = |items: &mut Vec<usize>| match settings.sort_by() {
            ::spel_katalog_settings::SortBy::Id => {
                items.sort_by_key(|idx| games.get(*idx).map(|game| -game.id))
            }
            ::spel_katalog_settings::SortBy::Name => {
                items.sort_by_key(|idx| games.get(*idx).map(|game| &game.name))
            }
            ::spel_katalog_settings::SortBy::Slug => {
                items.sort_by_key(|idx| games.get(*idx).map(|game| &game.slug))
            }
        };

        let mut to_be = if !filter.is_empty() {
            match settings.filter_mode() {
                ::spel_katalog_settings::FilterMode::Filter => {
                    let filter = filter.to_uppercase();

                    let mut to_be = to_be
                        .filter(|idx| {
                            let Some((_game, cache)) = get_game_cache(*idx) else {
                                return false;
                            };

                            cache.name.contains(&filter) || cache.slug.contains(&filter)
                        })
                        .collect();

                    sort_items(&mut to_be);

                    to_be
                }
                ::spel_katalog_settings::FilterMode::Search => {
                    let filter = filter.to_uppercase();
                    let mut dist = to_be
                        .filter_map(|idx| {
                            let (_, cache) = get_game_cache(idx)?;
                            Some((
                                idx,
                                cache.name.contains(&filter),
                                -::strsim::normalized_damerau_levenshtein(&filter, &cache.name),
                            ))
                        })
                        .collect::<Vec<_>>();

                    dist.sort_by(|a, b| a.1.cmp(&b.1).reverse().then(a.2.total_cmp(&b.2)));

                    dist.into_iter().map(|(idx, ..)| idx).collect()
                }
                ::spel_katalog_settings::FilterMode::Regex => to_be.collect(),
            }
        } else {
            let mut to_be = to_be.collect();

            sort_items(&mut to_be);

            to_be
        };

        if settings.sort_dir().is_reverse() {
            to_be.reverse();
        }

        *displayed = to_be;
    }

    fn by_slug_mut(&mut self, slug: &str) -> Option<&mut Game> {
        let idx = *self.slug_lookup.get(slug)?;
        self.games.get_mut(idx)
    }

    /// Get a game by it's id.
    pub fn by_id(&self, id: i64) -> Option<&Game> {
        let idx = *self.id_lookup.get(&id)?;
        self.games.get(idx)
    }

    /// Set the thumbnail of a game.
    pub fn set_image(&mut self, slug: &str, image: Handle) {
        if let Some(game) = self.by_slug_mut(slug) {
            game.image = Some(image);
        }
    }

    /// Set current games to the ones provided, then update lookups and display.
    pub fn set(&mut self, games: Box<[Game]>, settings: &Settings, filter: &str) {
        let (slug_lookup, id_lookup) = games
            .iter()
            .enumerate()
            .map(|(idx, game)| ((game.slug.clone(), idx), (game.id, idx)))
            .collect();
        let displayed = Vec::new();
        let cache = iter::repeat_with(OnceCell::new).take(games.len()).collect();

        *self = Self {
            games,
            slug_lookup,
            id_lookup,
            displayed,
            cache,
        };
        self.sort(settings, filter);
    }
}
