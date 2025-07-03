//! [State], [Message] and [Request] impls.

use ::std::{
    cell::Cell,
    convert::identity,
    io::Cursor,
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
use ::image::ImageFormat;
use ::itertools::Itertools;
use ::rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use ::rusqlite::{Connection, OpenFlags, named_params};
use ::rustc_hash::{FxHashMap, FxHashSet};
use ::spel_katalog_common::{OrRequest, StatusSender, async_status, status, w};
use ::spel_katalog_settings::{CacheDir, Settings};
use ::tap::Pipe;

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
        /// True if image comes from cache file.
        from_cache: bool,
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
    /// Select an id.
    SelectId(i64),
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
    Sqlite(#[from] ::rusqlite::Error),
}

fn load_db(path: &Path) -> Result<Vec<Game>, LoadDbError> {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let cats = db
        .prepare("SELECT id, name FROM categories")?
        .query_map([], |row| Ok((row.get("name")?, row.get("id")?)))?
        .collect::<Result<FxHashMap<String, i64>, ::rusqlite::Error>>();

    let hidden_cat = cats
        .as_ref()
        .as_ref()
        .ok()
        .and_then(|cats| cats.get(".hidden").copied())
        .unwrap_or(i64::MAX);

    let game_cats = db
        .prepare("SELECT game_id, category_id FROM games_categories")?
        .query_map([], |row| Ok((row.get("game_id")?, row.get("category_id")?)))?
        .fold(
            FxHashMap::<i64, Vec<i64>>::default(),
            |mut map, result| match result {
                Ok((game, cat)) => {
                    map.entry(game).or_default().push(cat);
                    map
                }
                Err(err) => {
                    ::log::error!("reading categories\n{err}");
                    map
                }
            },
        );

    let mut stmt = db.prepare("SELECT id,name,slug,runner,configpath FROM games")?;
    let mut rows = stmt.query([])?;
    let mut games = Vec::new();

    while let Some(row) = rows.next()? {
        let Some(mut game) = Game::from_row(row) else {
            continue;
        };

        if let Some(cats) = game_cats.get(&game.id) {
            if cats.iter().contains(&hidden_cat) {
                game.hidden = true;
            }
        }

        games.push(game);
    }

    games.sort_by_key(|game| -game.id);

    Ok(games)
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
        const THUMBNAILS_FILENAME: &str = "thumbnails.db";
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

                status!(tx, "read games from database");

                let game_slugs = self
                    .all()
                    .iter()
                    .map(|game| &game.slug)
                    .cloned()
                    .collect::<FxHashSet<_>>();
                let cache_dir = settings.get::<CacheDir>().to_path_buf();
                let find_cached = ::tokio::task::spawn_blocking(move || {
                    let thumbnail_cache_path = cache_dir.join(THUMBNAILS_FILENAME);
                    let db = match Connection::open_with_flags(
                        &thumbnail_cache_path,
                        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                    ) {
                        Ok(db) => db,
                        Err(err) => {
                            ::log::info!(
                                "could not open thumbnail cache {thumbnail_cache_path:?}\n{err}"
                            );
                            return Ok((None, game_slugs));
                        }
                    };

                    let mut stmt = db.prepare("SELECT slug, image FROM images")?;
                    let mut rows = stmt.query([])?;

                    fn get_slug_image(
                        row: &::rusqlite::Row<'_>,
                        thumbnail_cache_path: &Path,
                    ) -> Option<(String, Vec<u8>)> {
                        let slug = row
                            .get("slug")
                            .map_err(|err| {
                                ::log::error!(
                                    "could not get slug for row in {thumbnail_cache_path:?}\n{err}"
                                )
                            })
                            .ok()?;
                        let image = row
                            .get("image")
                            .map_err(|err| {
                                ::log::error!(
                                    "could not get image for row in {thumbnail_cache_path:?}\n{err}"
                                )
                            })
                            .ok()?;
                        Some((slug, image))
                    }

                    let mut slugs_bytes = Vec::new();
                    while let Some(row) = rows.next()? {
                        // if let Some((slug, handle)) =
                        //     read_row(row, &mut game_slugs, &thumbnail_cache_path)
                        // {
                        //     slugs.push(slug);
                        //     images.push(handle);
                        // }
                        slugs_bytes.extend(get_slug_image(row, &thumbnail_cache_path));
                    }
                    let mut slugs_images = Vec::new();
                    slugs_bytes.into_par_iter().map(|(slug, image)| {
                        if !game_slugs.contains(&slug) {
                            return None;
                        }
                        let image = match ::image::load_from_memory_with_format(&image, ImageFormat::Png) {
                            Ok(image) => image.into_rgba8(),
                            Err(err) => {
                                ::log::error!(
                                    "could not parse image for slug {slug} in {thumbnail_cache_path:?}\n{err}"
                                );
                                return None;
                            },
                        };

                        Some((slug, Handle::from_rgba(image.width(), image.height(), image.into_raw())))

                    }).collect_into_vec(&mut slugs_images);

                    let (slugs, images) = slugs_images.into_iter().flatten().unzip();

                    let mut game_slugs = game_slugs;
                    for slug in &slugs {
                        if !game_slugs.remove(slug) {
                            ::log::warn!(
                                "thumbnail for game with slug {slug} was present in thumbnail cache but not in game datatbase"
                            );
                        }
                    }

                    Ok::<_, ::rusqlite::Error>((
                        Some(Message::SetImages {
                            slugs,
                            images,
                            from_cache: true,
                        }),
                        game_slugs,
                    ))
                });

                Task::future(find_cached).then(|found| match found {
                    Ok(result) => match result {
                        Ok((set_images, slugs)) => Task::batch(
                            [
                                set_images.map(OrRequest::Message),
                                Request::FindImages {
                                    slugs: slugs.into_iter().collect::<Vec<_>>(),
                                }
                                .pipe(OrRequest::Request)
                                .pipe(Some),
                            ]
                            .into_iter()
                            .flatten()
                            .map(Task::done),
                        ),
                        Err(err) => {
                            ::log::error!("image cache collection failed\n{err}");
                            Task::none()
                        }
                    },
                    Err(err) => {
                        ::log::error!("image cache thread did not finish\n{err}");
                        Task::none()
                    }
                })
            }
            Message::SetImages {
                slugs,
                images,
                from_cache,
            } => {
                for (slug, image) in slugs.iter().zip(&images) {
                    self.set_image(slug, image.clone());
                }

                let cache_path = settings.get::<CacheDir>().to_path_buf();
                Task::future(async move {
                    if !from_cache {
                        if let Err(err) = ::tokio::fs::create_dir_all(&cache_path).await {
                            ::log::error!("could not create cache directory {cache_path:?}\n{err}");
                            return Ok(Ok(()));
                        }
                        let cache_path = cache_path.join(THUMBNAILS_FILENAME);
                        ::tokio::task::spawn_blocking(move || {
                            let db = Connection::open(&cache_path)?;

                            db.execute(
                                r#"
                                CREATE TABLE IF NOT EXISTS images(
                                    slug TEXT NOT NULL UNIQUE ON CONFLICT REPLACE,
                                    image BLOB NOT NULL
                                )
                                "#,
                                [],
                            )?;

                            let mut stmt = db.prepare(
                                r#"
                                INSERT INTO images (slug, image) VALUES (:slug, :image)   
                                "#,
                            )?;

                            let mut slugs_images = Vec::new();
                            slugs
                                .into_iter()
                                .zip(images)
                                .collect::<Vec<_>>()
                                .into_par_iter()
                                .map(|(slug, image)| {
                                    let Handle::Rgba {
                                        id: _,
                                        width,
                                        height,
                                        pixels,
                                    } = image
                                    else {
                                        return None;
                                    };
                                    let image =
                                        ::image::RgbaImage::from_raw(width, height, pixels.into())?;
                                    let mut buf = Vec::<u8>::new();

                                    if let Err(err) =
                                        image.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
                                    {
                                        ::log::error!(
                                            "failed to convert thumbnail for {slug} to png\n{err}"
                                        );
                                        return None;
                                    };

                                    Some((slug, buf))
                                })
                                .collect_into_vec(&mut slugs_images);

                            for (slug, image) in slugs_images.into_iter().flatten() {
                                if let Err(err) =
                                    stmt.execute(named_params! {":slug": slug, ":image": image})
                                {
                                    ::log::error!(
                                        "failed to save thumbnail for {slug} to cache\n{err}"
                                    );
                                }
                            }

                            Ok::<_, ::rusqlite::Error>(())
                        })
                        .await
                    } else {
                        Ok(Ok(()))
                    }
                })
                .then(|result| match result {
                    Ok(result) => match result {
                        Ok(_) => Task::none(),
                        Err(err) => {
                            ::log::error!("could not cache thumbnails\n{err}");
                            Task::none()
                        }
                    },
                    Err(err) => {
                        ::log::error!("cache thread failed\n{err}");
                        Task::none()
                    }
                })
            }
            Message::SetImage { slug, image } => {
                self.set_image(&slug, image);
                Task::none()
            }
            Message::Select(sel_dir) => {
                self.select(sel_dir);
                Task::none()
            }
            Message::SelectId(id) => {
                self.selected = Some(id);
                return Task::done(OrRequest::Request(Request::SetId { id }));
            }
        }
    }

    fn select(&mut self, sel_dir: SelDir) {
        use SelDir::*;
        let Some(selected) = self.selected else {
            self.selected = match sel_dir {
                Up | Left => self.displayed().next_back(),
                Down | Right => self.displayed().next(),
            }
            .map(|game| game.id);
            return;
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
    pub fn view(&self, shadowed: bool) -> Element<'_, OrRequest<Message, Request>> {
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
                    area.on_release(OrRequest::Message(Message::SelectId(id)))
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
            .id(widget::scrollable::Id::new("games-view"))
            .into()
        })
        .pipe(Element::from)
    }
}
