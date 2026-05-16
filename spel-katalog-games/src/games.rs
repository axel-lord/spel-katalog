//! [Games] impl.

use ::core::iter::{self, FusedIterator};

use ::derive_more::{Deref, DerefMut};
use ::itertools::izip;
use ::regex::RegexBuilder;
use ::rustc_hash::FxHashMap;
use ::spel_katalog_formats::{Game, GameId};
use ::spel_katalog_settings::{AsIndex, FilterMode, Settings, Show, SortBy, SortDir};
use ::tap::TapFallible;
use ::uuid::Uuid;

/// Cached game identity.
#[derive(Debug)]
struct GameCache {
    /// Game slug as uppercase.
    slug: Option<String>,
    /// Game name as uppercase.
    name: String,
}

impl From<&Game> for GameCache {
    fn from(game: &Game) -> Self {
        GameCache {
            slug: game.slug().map(|slug| slug.to_uppercase()),
            name: game.name().to_uppercase(),
        }
    }
}

/// A Game with an attached thumbnail.
#[derive(Debug, Deref, DerefMut)]
pub struct WithThumb {
    /// Game.
    #[deref]
    #[deref_mut]
    pub game: Game,
    /// Thumbnail.
    pub thumb: Option<::iced_widget::image::Handle>,
    /// Is the game batch selected.
    pub batch_selected: bool,
}

impl From<WithThumb> for Game {
    fn from(WithThumb { game, .. }: WithThumb) -> Self {
        game
    }
}

/// Collection of games.
#[derive(Debug, Default)]
pub struct Games {
    /// Cached game identities.
    cache: Box<[Option<GameCache>]>,
    /// Collection of game data and thumbnails.
    games: Box<[WithThumb]>,
    /// Indices of displayed games.
    displayed: Vec<usize>,
    /// Index lookup table by slug.
    slug_lookup: FxHashMap<String, usize>,
    /// Index lookup table by id.
    id_lookup: FxHashMap<i64, usize>,
    /// Index lookup table by uuid.
    uuid_lookup: FxHashMap<Uuid, usize>,
}

impl Games {
    /// Games that are currently to be displayed.
    pub fn displayed(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'_ WithThumb> + FusedIterator + Clone {
        self.displayed.iter().filter_map(|idx| self.games.get(*idx))
    }

    /// Games that are batch selected.
    pub fn batch_selected(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'_ WithThumb> + FusedIterator + Clone {
        self.games.iter().filter(|game| game.batch_selected)
    }

    /// All games.
    pub fn all(&self) -> &[WithThumb] {
        &self.games
    }

    /// All games as mutable.
    pub fn all_mut(&mut self) -> &mut [WithThumb] {
        &mut self.games
    }

    /// Amount of games.
    pub fn all_count(&self) -> usize {
        self.games.len()
    }

    /// Amount of displayed games.
    pub const fn displayed_count(&self) -> usize {
        self.displayed.len()
    }

    /// Sort displayed games.
    pub fn sort(&mut self, settings: &Settings, filter: &str) {
        let Self {
            games,
            displayed,
            slug_lookup: _,
            id_lookup: _,
            uuid_lookup: _,
            cache,
        } = self;

        fn get_filterend<'src>(
            games: &'src mut [WithThumb],
            cache: &'src mut [Option<GameCache>],
        ) -> Vec<(usize, &'src mut Game, &'src mut Option<GameCache>)> {
            izip!(0.., games, cache)
                .map(|(i, WithThumb { game, .. }, cache)| (i, game, cache))
                .collect()
        }

        fn filter_hidden<'a>(
            items: Vec<(usize, &'a mut Game, &'a mut Option<GameCache>)>,
            show: ::spel_katalog_settings::Show,
        ) -> Vec<(usize, &'a mut Game, &'a mut Option<GameCache>)> {
            match show {
                ::spel_katalog_settings::Show::Apparent => items
                    .into_iter()
                    .filter(|(_, game, _)| !game.hidden())
                    .collect(),
                ::spel_katalog_settings::Show::Hidden => items
                    .into_iter()
                    .filter(|(_, game, _)| game.hidden())
                    .collect(),
                ::spel_katalog_settings::Show::All => items,
            }
        }

        fn get_cache<'a>(game: &Game, cache: &'a mut Option<GameCache>) -> &'a GameCache {
            cache.get_or_insert_with(|| GameCache::from(game))
        }

        fn sort_items(
            items: &mut Vec<(usize, &mut Game, &mut Option<GameCache>)>,
            sort_by: SortBy,
            sort_dir: SortDir,
        ) {
            match sort_by {
                SortBy::Name => items.sort_by(|a, b| a.1.name().cmp(b.1.name())),
                SortBy::Added => {
                    items.sort_by(|a, b| a.1.installed_at().cmp(&b.1.installed_at()).reverse())
                }
            };

            if sort_dir.is_reverse() {
                items.reverse();
            }
        }

        if filter.trim().is_empty() {
            let mut filtered = get_filterend(games, cache);
            filtered = filter_hidden(filtered, settings[Show::as_idx()]);
            sort_items(
                &mut filtered,
                *settings.get::<SortBy>(),
                *settings.get::<SortDir>(),
            );
            *displayed = filtered.into_iter().map(|(i, ..)| i).collect();
            return;
        }

        match settings[FilterMode::as_idx()] {
            FilterMode::Filter => {
                if let Ok(filters) = ::shell_words::split(filter).tap_ok_mut(|filters| {
                    for filter in filters {
                        *filter = filter.to_uppercase();
                    }
                }) {
                    let mut filtered = get_filterend(games, cache);
                    filtered = filter_hidden(filtered, settings[Show::as_idx()]);
                    filtered = filtered
                        .into_iter()
                        .filter_map(|mut value| {
                            let (_, game, cache) = &mut value;
                            let cache = get_cache(game, cache);

                            for filter in &filters {
                                if cache.name.contains(filter) {
                                    continue;
                                }
                                if let Some(slug) = &cache.slug
                                    && slug.contains(filter)
                                {
                                    continue;
                                }
                                return None;
                            }

                            Some(value)
                        })
                        .collect();
                    sort_items(
                        &mut filtered,
                        *settings.get::<SortBy>(),
                        *settings.get::<SortDir>(),
                    );
                    *displayed = filtered.into_iter().map(|(i, ..)| i).collect();
                };
            }
            FilterMode::Search => {
                let mut filtered = get_filterend(games, cache);
                filtered = filter_hidden(filtered, settings[Show::as_idx()]);
                let filter = filter.to_uppercase();
                let mut dists = filtered
                    .iter_mut()
                    .map(|(idx, game, cache)| {
                        let cache = get_cache(game, cache);
                        (
                            *idx,
                            cache.name.contains(&filter),
                            -::strsim::normalized_damerau_levenshtein(&cache.name, &filter),
                        )
                    })
                    .collect::<Vec<_>>();
                dists.sort_by(|(_, contains_a, dist_a), (_, contains_b, dist_b)| {
                    contains_a
                        .cmp(contains_b)
                        .reverse()
                        .then(dist_a.total_cmp(dist_b))
                });

                if settings[SortDir::as_idx()].is_reverse() {
                    dists.reverse();
                }

                *displayed = dists.into_iter().map(|(i, ..)| i).collect();
            }
            FilterMode::Regex => {
                if let Ok(re) = RegexBuilder::new(filter).case_insensitive(true).build() {
                    let mut filtered = get_filterend(games, cache);
                    filtered = filter_hidden(filtered, settings[Show::as_idx()]);
                    filtered.retain(|(_, game, _)| re.is_match(game.name()));
                    sort_items(
                        &mut filtered,
                        *settings.get::<SortBy>(),
                        *settings.get::<SortDir>(),
                    );
                    *displayed = filtered.into_iter().map(|(i, ..)| i).collect();
                };
            }
        }
    }

    /// Get a mutable reference to a game with the given slug.
    fn by_slug_mut(&mut self, slug: &str) -> Option<&mut WithThumb> {
        let idx = *self.slug_lookup.get(slug)?;
        self.games.get_mut(idx)
    }

    /// Lookup index from id.
    fn id_lookup(&self, id: GameId) -> Option<usize> {
        match id {
            GameId::Lutris(id) => self.id_lookup.get(&id),
            GameId::Native(uuid) => self.uuid_lookup.get(&uuid),
        }
        .copied()
    }

    /// Get a game by it's id.
    pub fn by_id(&self, id: GameId) -> Option<&WithThumb> {
        let idx = self.id_lookup(id)?;
        self.games.get(idx)
    }

    /// Get a game by it's id as mutable.
    pub fn by_id_mut(&mut self, id: GameId) -> Option<&mut WithThumb> {
        let idx = self.id_lookup(id)?;
        self.games.get_mut(idx)
    }

    /// Set the thumbnail of a game.
    pub(crate) fn set_image(&mut self, slug: &str, image: ::spel_katalog_formats::Image) {
        if let Some(game) = self.by_slug_mut(slug) {
            game.thumb = Some(::iced_widget::image::Handle::from_rgba(
                image.width,
                image.height,
                image.bytes,
            ))
        }
    }

    /// Remove the thumbnail of a game.
    pub(crate) fn remove_image(&mut self, slug: &str) {
        if let Some(game) = self.by_slug_mut(slug) {
            game.thumb = None;
        }
    }

    /// Set current games to the ones provided, then update lookups and display.
    pub fn set(&mut self, games: Box<[WithThumb]>, settings: &Settings, filter: &str) {
        let mut slug_lookup = FxHashMap::default();
        let mut id_lookup = FxHashMap::default();
        let mut uuid_lookup = FxHashMap::default();
        for (idx, game) in games.iter().enumerate() {
            match &game.game {
                Game::Lutris(lutris_game) => {
                    slug_lookup.insert(lutris_game.slug.clone(), idx);
                    id_lookup.insert(lutris_game.id, idx);
                }
                Game::Native { uuid, .. } => {
                    uuid_lookup.insert(*uuid, idx);
                }
            }
        }
        let displayed = Vec::new();
        let cache = iter::repeat_with(|| None).take(games.len()).collect();

        *self = Self {
            games,
            slug_lookup,
            id_lookup,
            uuid_lookup,
            displayed,
            cache,
        };
        self.sort(settings, filter);
    }
}
