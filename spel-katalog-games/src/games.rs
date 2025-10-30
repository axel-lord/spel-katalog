//! [Games] impl.

use ::std::iter::{self, FusedIterator};

use ::derive_more::{Deref, DerefMut};
use ::itertools::izip;
use ::regex::RegexBuilder;
use ::rustc_hash::FxHashMap;
use ::spel_katalog_formats::Game;
use ::spel_katalog_settings::{AsIndex, FilterMode, Settings, Show, SortBy, SortDir};
use ::tap::TapFallible;

#[derive(Debug)]
struct GameCache {
    slug: String,
    name: String,
}

/// A Game with an attached thumbnail.
#[derive(Debug, Deref, DerefMut)]
pub struct WithThumb {
    /// Game.
    #[deref]
    #[deref_mut]
    pub game: Game,
    /// Thumbnail.
    pub thumb: Option<::iced::advanced::image::Handle>,
}

impl From<Game> for WithThumb {
    fn from(game: Game) -> Self {
        Self { game, thumb: None }
    }
}

impl From<WithThumb> for Game {
    fn from(WithThumb { game, .. }: WithThumb) -> Self {
        game
    }
}

/// Collection of games.
#[derive(Debug, Default)]
pub struct Games {
    cache: Box<[Option<GameCache>]>,
    games: Box<[WithThumb]>,
    displayed: Vec<usize>,
    slug_lookup: FxHashMap<String, usize>,
    id_lookup: FxHashMap<i64, usize>,
}

impl Games {
    /// Games that are currently to be displayed.
    pub fn displayed(
        &self,
    ) -> impl Iterator<Item = &'_ WithThumb> + DoubleEndedIterator + FusedIterator + Clone {
        self.displayed.iter().filter_map(|idx| self.games.get(*idx))
    }

    /// Games that are batch selected.
    pub fn batch_selected(
        &self,
    ) -> impl Iterator<Item = &'_ WithThumb> + DoubleEndedIterator + FusedIterator + Clone {
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

        let mut to_be = izip!(0.., games, cache)
            .map(|(i, WithThumb { game, .. }, cache)| (i, game, cache))
            .collect::<Vec<_>>();

        fn filter_hidden<'a>(
            to_be: Vec<(usize, &'a mut Game, &'a mut Option<GameCache>)>,
            show: ::spel_katalog_settings::Show,
        ) -> Vec<(usize, &'a mut Game, &'a mut Option<GameCache>)> {
            match show {
                ::spel_katalog_settings::Show::Apparent => to_be
                    .into_iter()
                    .filter(|(_, game, _)| !game.hidden)
                    .collect(),
                ::spel_katalog_settings::Show::Hidden => to_be
                    .into_iter()
                    .filter(|(_, game, _)| game.hidden)
                    .collect(),
                ::spel_katalog_settings::Show::All => to_be,
            }
        }

        fn get_cache<'a>(game: &Game, cache: &'a mut Option<GameCache>) -> &'a GameCache {
            cache.get_or_insert_with(|| GameCache {
                slug: game.slug.to_uppercase(),
                name: game.name.to_uppercase(),
            })
        }

        match settings[FilterMode::as_idx()] {
            FilterMode::Filter => {
                let Ok(filters) = ::shell_words::split(filter).tap_ok_mut(|filters| {
                    for filter in filters {
                        *filter = filter.to_uppercase();
                    }
                }) else {
                    return;
                };
                to_be = filter_hidden(to_be, settings[Show::as_idx()]);
                to_be = to_be
                    .into_iter()
                    .filter_map(|mut value| {
                        let (_, game, cache) = &mut value;
                        let cache = get_cache(game, cache);

                        for filter in &filters {
                            if cache.name.contains(filter) || cache.slug.contains(filter) {
                                continue;
                            }
                            return None;
                        }

                        Some(value)
                    })
                    .collect();
            }
            FilterMode::Search => {
                let filter = filter.to_uppercase();
                to_be = filter_hidden(to_be, settings[Show::as_idx()]);
                let mut dists = to_be
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

                // Search early returns here.
                return;
            }
            FilterMode::Regex => {
                let Ok(re) = RegexBuilder::new(filter).case_insensitive(true).build() else {
                    return;
                };
                to_be = filter_hidden(to_be, settings[Show::as_idx()]);
                to_be = to_be
                    .into_iter()
                    .filter(|(_, game, _)| re.is_match(&game.name))
                    .collect();
            }
        }

        match settings[SortBy::as_idx()] {
            SortBy::Id => to_be.sort_by(|a, b| a.1.id.cmp(&b.1.id).reverse()),
            SortBy::Name => to_be.sort_by(|a, b| a.1.name.cmp(&b.1.name)),
            SortBy::Slug => to_be.sort_by(|a, b| a.1.slug.cmp(&b.1.slug)),
        };

        if settings[SortDir::as_idx()].is_reverse() {
            to_be.reverse();
        }

        *displayed = to_be.into_iter().map(|(i, ..)| i).collect();
    }

    fn by_slug_mut(&mut self, slug: &str) -> Option<&mut WithThumb> {
        let idx = *self.slug_lookup.get(slug)?;
        self.games.get_mut(idx)
    }

    /// Get a game by it's id.
    pub fn by_id(&self, id: i64) -> Option<&WithThumb> {
        let idx = *self.id_lookup.get(&id)?;
        self.games.get(idx)
    }

    /// Get a game by it's id as mutable.
    pub fn by_id_mut(&mut self, id: i64) -> Option<&mut WithThumb> {
        let idx = *self.id_lookup.get(&id)?;
        self.games.get_mut(idx)
    }

    /// Set the thumbnail of a game.
    pub(crate) fn set_image(&mut self, slug: &str, image: ::spel_katalog_formats::Image) {
        if let Some(game) = self.by_slug_mut(slug) {
            game.thumb = Some(::iced::advanced::image::Handle::from_rgba(
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
        let (slug_lookup, id_lookup) = games
            .iter()
            .enumerate()
            .map(|(idx, game)| ((game.slug.clone(), idx), (game.id, idx)))
            .collect();
        let displayed = Vec::new();
        let cache = iter::repeat_with(|| None).take(games.len()).collect();

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
