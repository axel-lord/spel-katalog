//! [Games] impl.

use ::core::{iter::FusedIterator, mem};

use ::derive_more::{Deref, DerefMut, IsVariant};
use ::itertools::izip;
use ::regex::RegexBuilder;
use ::rustc_hash::FxHashMap;
use ::spel_katalog_formats::{Game, GameId, NativeGame};
use ::spel_katalog_settings::{AsIndex, FilterMode, Settings, Show, SortBy, SortDir};
use ::tap::TapFallible;
use ::uuid::Uuid;

/// Result of trying to add a game.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, IsVariant)]
pub enum GameAddDelta {
    /// Game existed and was updated, refresh of games required.
    Refresh = 5,
    /// Game was not present and thus added.
    /// It may have shadowed another game.
    Added = 4,
    /// Game existed and was updated.
    Updated = 3,
    /// Game is shadowed and thus not added.
    Skipped = 2,
    /// Game is a ghost and thus not added.
    Ghost = 1,
}

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
    pub thumb: Option<::iced_core::image::Handle>,
    /// Thumbnail preview.
    pub thumb_thumb: Option<::iced_core::image::Handle>,
    /// Is the game batch selected.
    pub batch_selected: bool,
    /// This game shadows another game with given id.
    pub shadows: Option<GameId>,
    /// This game will be removed on the next refresh.
    pub ghost: bool,
}

impl From<WithThumb> for Game {
    fn from(WithThumb { game, .. }: WithThumb) -> Self {
        game
    }
}

impl From<(Uuid, NativeGame)> for WithThumb {
    fn from((uuid, game): (Uuid, NativeGame)) -> Self {
        Self {
            game: Game::Native {
                name: game.name,
                installed_at: game.timestamp.timestamp(),
                uuid,
                hidden: game.hidden,
            },
            thumb: None,
            batch_selected: false,
            shadows: game.shadow,
            ghost: false,
            thumb_thumb: None,
        }
    }
}

/// Iterator removing games matching filter.
#[derive(Debug)]
pub struct RemoveGames<'g, F> {
    /// Condition for removing game.
    cond: F,
    /// Reference to games.
    games: &'g mut Games,
    /// Current index.
    idx: usize,
    /// Settings reference.
    settings: &'g Settings,
    /// Filter to use when sorting.
    filter: &'g str,
}

impl<F> Iterator for RemoveGames<'_, F>
where
    F: for<'a> FnMut(&'a WithThumb) -> bool,
{
    type Item = WithThumb;

    fn next(&mut self) -> Option<Self::Item> {
        let Self {
            cond, games, idx, ..
        } = self;
        loop {
            if cond(games.games.get(*idx)?) {
                let mut replacement = games.games.pop()?;
                if let Some(elem) = games.games.get_mut(*idx) {
                    mem::swap(&mut replacement, elem);
                }
                return Some(replacement);
            } else {
                // Move forward only if game was not removed.
                *idx += 1;
            }
        }
    }
}
impl<F> FusedIterator for RemoveGames<'_, F> where F: for<'a> FnMut(&'a WithThumb) -> bool {}

impl<F> Drop for RemoveGames<'_, F> {
    fn drop(&mut self) {
        self.games.refresh(self.settings, self.filter);
    }
}

/// Collection of games.
#[derive(Debug, Default)]
pub struct Games {
    /// Cached game identities.
    cache: Vec<Option<GameCache>>,
    /// Collection of game data and thumbnails.
    games: Vec<WithThumb>,
    /// Indices of displayed games.
    displayed: Vec<usize>,
    /// Index lookup table by slug.
    slug_lookup: FxHashMap<String, usize>,
    /// Index lookup table by id.
    id_lookup: FxHashMap<i64, usize>,
    /// Index lookup table by uuid.
    uuid_lookup: FxHashMap<Uuid, usize>,
    /// Last state of hidden setting.
    last_show: Option<Show>,
}

impl Games {
    /// Clear all state in use.
    pub fn clear(&mut self) {
        let Self {
            cache,
            games,
            displayed,
            slug_lookup,
            id_lookup,
            uuid_lookup,
            last_show,
        } = self;
        cache.clear();
        games.clear();
        displayed.clear();
        id_lookup.clear();
        uuid_lookup.clear();
        slug_lookup.clear();
        *last_show = None;
    }

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
    pub const fn all_count(&self) -> usize {
        self.games.len()
    }

    /// Amount of displayed games.
    pub const fn displayed_count(&self) -> usize {
        self.displayed.len()
    }

    /// Remove all games matching condition.
    ///
    /// Games are only removed from current session.
    pub const fn remove_games<'g, F>(
        &'g mut self,
        cond: F,
        settings: &'g Settings,
        filter: &'g str,
    ) -> RemoveGames<'g, F>
    where
        F: for<'a> FnMut(&'a WithThumb) -> bool,
    {
        RemoveGames {
            cond,
            games: self,
            idx: 0,
            settings,
            filter,
        }
    }

    /// Remove a single game.
    ///
    /// Game is only removed for current session.
    pub fn remove_game(
        &mut self,
        id: GameId,
        settings: &Settings,
        filter: &str,
    ) -> Option<WithThumb> {
        let idx = self.id_lookup(id)?;
        let mut replacement = self.games.pop()?;
        if let Some(game) = self.games.get_mut(idx) {
            mem::swap(&mut replacement, game);
        }
        self.refresh(settings, filter);
        Some(replacement)
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
            last_show,
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
            show: Show,
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

        let show = *settings.get::<Show>();

        if let Some(last_show) = *last_show
            && !show.is_all()
            && show != last_show
        {
            ::log::info!("show {last_show} -> {show}, removing thumbnails");
            for game in games.iter_mut() {
                if let GameId::Native(_uuid) = game.id() {
                    game.thumb = None
                }
            }
        }
        *last_show = Some(show);

        if filter.trim().is_empty() {
            let mut filtered = get_filterend(games, cache);
            filtered = filter_hidden(filtered, show);
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

    /// Add id or replace it, pointing to idx.
    fn id_add_replace(&mut self, id: GameId, idx: usize) {
        match id {
            GameId::Lutris(id) => self.id_lookup.insert(id, idx),
            GameId::Native(uuid) => self.uuid_lookup.insert(uuid, idx),
        };
    }

    /// Get a game by it's id.
    pub fn by_id(&self, id: GameId) -> Option<&WithThumb> {
        let idx = self.id_lookup(id)?;
        self.games.get(idx)
    }

    /// Get game by uuid.
    pub fn by_uuid(&self, uuid: Uuid) -> Option<&WithThumb> {
        let idx = self.uuid_lookup.get(&uuid)?;
        self.games.get(*idx)
    }

    /// Get a game by it's id as mutable.
    pub fn by_id_mut(&mut self, id: GameId) -> Option<&mut WithThumb> {
        let idx = self.id_lookup(id)?;
        self.games.get_mut(idx)
    }

    /// Get game by uuid.
    pub fn by_uuid_mut(&mut self, uuid: Uuid) -> Option<&mut WithThumb> {
        let idx = self.uuid_lookup.get(&uuid)?;
        self.games.get_mut(*idx)
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

    /// Add a new game.
    fn add_game_new(&mut self, game: WithThumb) {
        let game_id = game.id();
        if let Some(shadows) = game.shadows
            && let Some(shadowed_idx) = self.id_lookup(shadows)
            && let Some(shadowed_cache) = self.cache.get_mut(shadowed_idx)
            && let Some(shadowed_game) = self.games.get_mut(shadowed_idx)
        {
            if let Some(shadowed_slug) = shadowed_game.slug() {
                self.slug_lookup.remove(shadowed_slug);
            }
            if let Some(slug) = game.slug() {
                self.slug_lookup.insert(slug.to_owned(), shadowed_idx);
            }
            *shadowed_cache = Some(GameCache::from(&*game));
            *shadowed_game = game;
            self.id_add_replace(game_id, shadowed_idx);
        } else {
            let idx = self.games.len();
            let shadows = game.shadows;
            if let Some(slug) = game.slug() {
                self.slug_lookup.insert(slug.to_owned(), idx);
            }
            self.cache.push(Some(GameCache::from(&*game)));
            self.games.push(game);
            self.id_add_replace(game_id, idx);
            if let Some(shadows) = shadows {
                self.id_add_replace(shadows, idx);
            }
        }
    }

    /// Add game to games state.
    pub fn add_game(&mut self, game: WithThumb) -> GameAddDelta {
        let game_id = game.id();
        if game.ghost {
            // Game should not be added.
            GameAddDelta::Ghost
        } else if let Some(occupied_idx) = self.id_lookup(game_id)
            && let Some(occupied_game) = self.games.get_mut(occupied_idx)
            && let Some(occupied_cache) = self.cache.get_mut(occupied_idx)
        {
            if game_id == occupied_game.id() {
                let refresh = game.shadows.is_some() && game.shadows != occupied_game.shadows;
                *occupied_cache = Some(GameCache::from(&*game));
                *occupied_game = game;
                // This game existed and was updated.
                if refresh {
                    GameAddDelta::Refresh
                } else {
                    GameAddDelta::Updated
                }
            } else if occupied_game.shadows == Some(game_id) {
                // This game is shadowed.
                GameAddDelta::Skipped
            } else {
                self.add_game_new(game);
                // Added game as new, will correct wrong lookups, and deal with shadowing.
                GameAddDelta::Added
            }
        } else {
            self.add_game_new(game);
            // Added game as new, will deal with shadowing.
            GameAddDelta::Added
        }
    }

    /// Add multiple games, refreshing if needed.
    pub fn add_games(
        &mut self,
        games: impl IntoIterator<Item = WithThumb>,
        settings: &Settings,
        filter: &str,
    ) {
        let mut delta = GameAddDelta::Skipped;
        for game in games {
            delta = self.add_game(game).max(delta);
        }

        if delta.is_refresh() {
            self.refresh(settings, filter);
        } else {
            self.sort(settings, filter);
        }
    }

    /// Refresh games state.
    pub fn refresh(&mut self, settings: &Settings, filter: &str) {
        let games = mem::take(&mut self.games);
        for game in games {
            if self.add_game(game).is_refresh() {
                ::log::warn!("possible refresh cycle shadowed games, refresh required on refresh")
            }
        }
        self.sort(settings, filter);
    }

    /// Set current games to the ones provided, then update lookups and display.
    pub fn set(&mut self, games: Vec<WithThumb>, settings: &Settings, filter: &str) {
        self.clear();
        self.add_games(games, settings, filter);
    }
}
