//! [State], [Message] and [Request] impls.

use ::std::{
    cell::Cell,
    convert::identity,
    path::{Path, PathBuf},
};

use ::derive_more::{Deref, DerefMut, IsVariant};
use ::iced::{
    Alignment::{self},
    Element,
    Length::Fill,
    Task,
    widget::{self, container, image::Handle, stack},
};
use ::itertools::Itertools;
use ::rustc_hash::FxHashMap;
use ::spel_katalog_common::{OrRequest, StatusSender, async_status, status, w};
use ::spel_katalog_settings::Settings;
use ::tap::{Pipe, Tap, TapOptional};

use crate::{Game, Games};

/// State of games element.
#[derive(Debug, Default, Deref, DerefMut)]
pub struct State {
    #[deref]
    #[deref_mut]
    games: Games,
    selected: Option<i64>,
    columns: Cell<usize>,
}

/// What direction to select element in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, IsVariant)]
pub enum SelDir {
    /// Select element above current.
    Up,
    /// Select element bellow current.
    Down,
    /// Select element to the left of current.
    Left,
    /// Select element to the right of current.
    Right,
}

/// Internal message used for games element.
#[derive(Debug, Clone, IsVariant)]
pub enum Message {
    /// Load games from local lutris database.
    LoadDb(PathBuf),
    /// Set loaded games.
    SetGames(Vec<Game>),
    /// Set thumbnails.
    SetImages {
        /// Slugs for games to set thumbnails for.
        slugs: Vec<String>,
        /// Thumbnails to set.
        images: Vec<Handle>,
    },
    /// Set a single thumbnail.
    SetImage {
        /// Slug.
        slug: String,
        /// Image.
        image: Handle,
    },
    /// Move selection.
    Select(SelDir),
}

/// Requests for other widgets.
#[derive(Debug, Clone, IsVariant)]
pub enum Request {
    /// Set currently chosen game.
    SetId {
        /// Id of game
        id: i64,
    },
    /// Run a game.
    Run {
        /// Id of game.
        id: i64,
        /// Should the game be sandboxed.
        sandbox: bool,
    },
    /// Find thumbnails.
    FindImages {
        /// Slugs of thumbnails to find.
        slugs: Vec<String>,
    },
}

#[derive(Debug, ::thiserror::Error)]
pub enum LoadDbError {
    #[error("an sqlite error occurred\n{0}")]
    Sqlite(#[from] ::sqlite::Error),
}

fn load_db(path: &Path) -> Result<Vec<Game>, LoadDbError> {
    let db = ::sqlite::open(path)?;

    let cats = db
        .prepare("SELECT id, name FROM categories")?
        .into_iter()
        .map(|cat| {
            let cat = cat?;
            let id = cat.try_read::<i64, _>("id")?;
            let name = cat.try_read::<&str, _>("name")?;

            Ok((String::from(name), id))
        })
        .collect::<Result<FxHashMap<_, _>, ::sqlite::Error>>();

    let hidden_cat = cats
        .as_ref()
        .ok()
        .and_then(|cats| cats.get(".hidden").cloned())
        .unwrap_or(i64::MAX);

    let game_cats = db
        .prepare("SELECT game_id, category_id FROM games_categories")?
        .into_iter()
        .map(|row| {
            let row = row?;
            let game: i64 = row.try_read("game_id")?;
            let cat: i64 = row.try_read("category_id")?;

            Ok::<_, ::sqlite::Error>((game, cat))
        })
        .fold(FxHashMap::<i64, Vec<i64>>::default(), |mut map, result| {
            let (game, cat) = match result {
                Ok(values) => values,
                Err(err) => {
                    ::log::error!("reading categories\n{err}");
                    return map;
                }
            };
            map.entry(game).or_default().push(cat);
            map
        });

    db.prepare("SELECT id,name,slug,runner,configpath FROM games")?
        .into_iter()
        .filter_map(|row| {
            let row = row
                .map_err(|err| ::log::error!("row does not exist\n{err}"))
                .ok()?;

            let game = Game::from_row(&row).tap_some_mut(|game| {
                let Some(cats) = game_cats.get(&game.id) else {
                    return;
                };

                if cats.iter().contains(&hidden_cat) {
                    game.hidden = true;
                }
            });
            game
        })
        .collect::<Vec<_>>()
        .tap_mut(|games| games.sort_by_key(|game| -game.id))
        .pipe(Ok)
}

impl State {
    /// Get current amount of columns.
    pub fn columns(&self) -> usize {
        self.columns.get()
    }

    /// Get id of currently selected game, if any.
    pub fn selected(&self) -> Option<i64> {
        self.selected
    }

    /// Update internal state and send messages.
    pub fn update(
        &mut self,
        msg: Message,
        tx: &StatusSender,
        settings: &Settings,
        filter: &str,
    ) -> Task<OrRequest<Message, Request>> {
        match msg {
            Message::LoadDb(path_buf) => {
                let tx = tx.clone();
                Task::future(async move {
                    match ::tokio::task::spawn_blocking(move || load_db(&path_buf)).await {
                        Ok(result) => match result {
                            Ok(games) => games
                                .pipe(Message::SetGames)
                                .pipe(OrRequest::Message)
                                .pipe(Task::done),
                            Err(err) => match err {
                                LoadDbError::Sqlite(error) => {
                                    ::log::error!("an sqlite error occurred\n{error}");
                                    async_status!(tx, "an sqlite error occurred").await;
                                    Task::none()
                                }
                            },
                        },
                        Err(err) => {
                            ::log::error!("database thread did not finish\n{err}");
                            async_status!(tx, "thread did not finish").await;
                            Task::none()
                        }
                    }
                })
                .then(identity)
            }
            Message::SetGames(games) => {
                self.set(games.into(), settings, filter);

                let slugs = self.all().iter().map(|game| &game.slug).cloned().collect();
                status!(tx, "read games from database");

                Task::done(OrRequest::Request(Request::FindImages { slugs }).into())
            }
            Message::SetImages { slugs, images } => {
                for (slug, image) in slugs.into_iter().zip(images) {
                    self.set_image(&slug, image);
                }

                Task::none()
            }
            Message::SetImage { slug, image } => {
                self.set_image(&slug, image);
                Task::none()
            }
            Message::Select(sel_dir) => {
                self.select(sel_dir);
                Task::none()
            }
        }
    }

    fn select(&mut self, sel_dir: SelDir) {
        use SelDir::*;
        let Some(selected) = self.selected else {
            return self.selected = match sel_dir {
                Up | Left => self.displayed().next_back(),
                Down | Right => self.displayed().next(),
            }
            .map(|game| game.id);
        };

        let m = |game: &Game| game.id == selected;

        let idx = match sel_dir {
            Up | Left => self.displayed().rev().position(m),
            Down | Right => self.displayed().position(m),
        };

        let Some(idx) = idx else {
            self.selected = None;
            self.select(sel_dir);
            return;
        };

        self.selected = match sel_dir {
            Up => self
                .displayed()
                .rev()
                .cycle()
                .skip(idx + self.columns())
                .next(),
            Down => self.displayed().cycle().skip(idx + self.columns()).next(),
            Left => self.displayed().rev().cycle().skip(idx + 1).next(),
            Right => self.displayed().cycle().skip(idx + 1).next(),
        }
        .map(|game| game.id);
    }

    /// Render elements.
    pub fn view(&self, shadowed: bool) -> Element<OrRequest<Message, Request>> {
        fn card<'a>(
            game: &'a Game,
            width: f32,
            shadowed: bool,
            selected: Option<i64>,
        ) -> Element<'a, OrRequest<Message, Request>> {
            let handle = game.image.as_ref();
            let name = game.name.as_str();
            let id = game.id;

            let style: fn(&::iced::Theme) -> container::Style;
            if selected == Some(id) {
                style = |theme| container::bordered_box(theme).background(theme.palette().primary);
            } else {
                style = container::bordered_box;
            };

            let text = container(name)
                .padding(3)
                .style(style)
                .pipe(container)
                .width(width)
                .height(width)
                .padding(3)
                .align_x(Alignment::Center)
                .align_y(Alignment::End);

            match handle {
                Some(handle) => {
                    let image = widget::image(handle)
                        .width(width)
                        .height(width)
                        .content_fit(::iced::ContentFit::Contain);
                    widget::mouse_area(stack([image.into(), text.into()]))
                }
                None => widget::mouse_area(text),
            }
            .interaction(if shadowed {
                ::iced::mouse::Interaction::default()
            } else {
                ::iced::mouse::Interaction::Pointer
            })
            .pipe(|area| {
                if shadowed {
                    area
                } else {
                    area.on_release(OrRequest::Request(Request::SetId { id }))
                        .on_middle_release(OrRequest::Request(Request::Run { id, sandbox: true }))
                }
            })
            .into()
        }

        widget::responsive(move |size| {
            let columns = ((size.width as usize - 1) / 153).clamp(1, 24);
            let width = ((size.width / columns as f32) - 3.0).clamp(150.0, 300.0);
            self.columns.set(columns);

            w::scroll(w::col().align_x(Alignment::Start).width(Fill).extend(
                self.displayed().chunks(columns).into_iter().map(|chunk| {
                    w::row()
                        .extend(chunk.map(|game| card(game, width, shadowed, self.selected)))
                        .into()
                }),
            ))
            .into()
        })
        .pipe(Element::from)
    }
}
