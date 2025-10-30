//! [State], [Message] and [Request] impls.

use ::std::{
    cell::Cell,
    convert::identity,
    io::Cursor,
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use ::derive_more::{Deref, DerefMut, IsVariant};
use ::iced::{
    Alignment::{self},
    Border, Element,
    Length::Fill,
    Task,
    widget::{self, container, stack},
};
use ::image::ImageFormat;
use ::itertools::Itertools;
use ::rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use ::rusqlite::{Connection, OpenFlags, Statement, named_params};
use ::rustc_hash::FxHashSet;
use ::spel_katalog_common::{OrRequest, StatusSender, async_status, status, w};
use ::spel_katalog_formats::Game;
use ::spel_katalog_gather::{LoadDbError, load_games_from_database};
use ::spel_katalog_settings::{CacheDir, Settings};
use ::spel_katalog_tracker::Tracker;
use ::tap::Pipe;
use ::tokio::task::{JoinError, spawn_blocking};

use crate::{Games, games::WithThumb};

const THUMBNAILS_FILENAME: &str = "thumbnails.db";

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
    /// Deselect element.
    None,
}

/// Internal message used for games element.
#[derive(Debug, IsVariant)]
pub enum Message {
    /// Load games from local lutris database.
    LoadDb {
        /// Path to database to load.
        db_path: PathBuf,
        /// Tracker to activate when loading has finished.
        tracker: Option<Tracker>,
    },
    /// Set loaded games.
    SetGames {
        /// Games to set content to.
        games: Vec<Game>,
        /// Tracker to activate on finished loading.
        tracker: Option<Tracker>,
    },
    /// Set thumbnails.
    SetImages {
        /// Slugs for games to set thumbnails for.
        slugs: Vec<String>,
        /// Thumbnails to set.
        images: Vec<::spel_katalog_formats::Image>,
        /// True if image comes from cache file.
        from_cache: bool,
        /// Tracker to activate when finished.
        tracker: Option<Tracker>,
    },
    /// Set a single thumbnail.
    SetImage {
        /// Slug.
        slug: String,
        /// Image.
        image: ::spel_katalog_formats::Image,
    },
    /// Remove a thumbnail from game and cache.
    RemoveImage {
        /// Game slug.
        slug: String,
    },
    /// Move selection.
    Select(SelDir),
    /// Select an id.
    SelectId(i64),
    /// Batch select game.
    BatchSelect(i64),
}

/// Requests for other widgets.
#[derive(Debug, IsVariant)]
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
        /// Tracker to activate when finished.
        tracker: Option<Tracker>,
    },
    /// Close game info
    CloseInfo,
}

/// Messages produced by game areas.
#[derive(Debug, Clone, Copy)]
pub enum AreaMessage {
    Select { id: i64 },
    BatchSelect { id: i64 },
    Run { id: i64, sandbox: bool },
}

impl From<AreaMessage> for OrRequest<Message, Request> {
    fn from(value: AreaMessage) -> Self {
        match value {
            AreaMessage::Select { id } => OrRequest::Message(Message::SelectId(id)),
            AreaMessage::BatchSelect { id } => OrRequest::Message(Message::BatchSelect(id)),
            AreaMessage::Run { id, sandbox } => OrRequest::Request(Request::Run { id, sandbox }),
        }
    }
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
            Message::LoadDb { db_path, tracker } => {
                let tx = tx.clone();
                Task::future(async move {
                    match spawn_blocking(move || load_games_from_database(&db_path)).await {
                        Ok(result) => match result {
                            Ok(games) => games
                                .pipe(|games| Message::SetGames { games, tracker })
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
            Message::SetGames { games, tracker } => {
                self.set(
                    games.into_iter().map(WithThumb::from).collect(),
                    settings,
                    filter,
                );

                status!(tx, "read games from database");

                self.find_cached(settings, tracker)
            }
            Message::SetImages {
                slugs,
                images,
                from_cache,
                tracker,
            } => {
                if from_cache {
                    for (slug, image) in slugs.into_iter().zip(images) {
                        self.set_image(&slug, image);
                    }
                    Task::none()
                } else {
                    for (slug, image) in slugs.iter().zip(&images) {
                        self.set_image(slug, image.clone());
                    }
                    let cache_path = settings.get::<CacheDir>().to_path_buf();
                    Task::future(cache_images(slugs, images, cache_path, tx.clone(), tracker))
                        .then(|_| Task::none())
                }
            }
            Message::SetImage { slug, image } => {
                self.set_image(&slug, image.clone());
                let cache_path = settings.get::<CacheDir>().to_path_buf();
                Task::future(cache_image(slug, image, cache_path, tx.clone()))
                    .then(|_| Task::none())
            }
            Message::RemoveImage { slug } => {
                self.remove_image(&slug);
                let cache_path = settings.get::<CacheDir>().to_path_buf();
                Task::future(uncache_image(slug, cache_path)).then(|_| Task::none())
            }
            Message::Select(sel_dir) => {
                self.select(sel_dir);
                Task::none()
            }
            Message::SelectId(id) => {
                self.selected = Some(id);
                return Task::done(OrRequest::Request(Request::SetId { id }));
            }
            Message::BatchSelect(id) => {
                if let Some(game) = self.games.by_id_mut(id) {
                    game.batch_selected = !game.batch_selected;
                }
                Task::none()
            }
        }
    }

    /// Find cached images.
    pub fn find_cached(
        &mut self,
        settings: &Settings,
        mut tracker: Option<Tracker>,
    ) -> Task<OrRequest<Message, Request>> {
        let game_slugs = self
            .all()
            .iter()
            .map(|game| &game.slug)
            .cloned()
            .collect::<FxHashSet<_>>();
        let cache_dir = settings.get::<CacheDir>().to_path_buf();
        let find_cached = spawn_blocking(move || {
            let thumbnail_cache_path = cache_dir.join(THUMBNAILS_FILENAME);
            let db = match Connection::open_with_flags(
                &thumbnail_cache_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            ) {
                Ok(db) => db,
                Err(err) => {
                    ::log::info!("could not open thumbnail cache {thumbnail_cache_path:?}\n{err}");
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

                                Some((slug, ::spel_katalog_formats::Image { width: image.width(), height: image.height(), bytes: image.into_raw().into() }))

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
                    tracker: None,
                }),
                game_slugs,
            ))
        });

        Task::future(find_cached).then(move |found| match found {
            Ok(result) => match result {
                Ok((set_images, slugs)) => Task::batch(
                    [
                        set_images.map(OrRequest::Message),
                        Request::FindImages {
                            slugs: slugs.into_iter().collect::<Vec<_>>(),
                            tracker: tracker.take(),
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

    /// Deselect all batch selected games.
    pub fn deselect_batch(&mut self) {
        for game in self.all_mut() {
            if game.batch_selected {
                game.batch_selected = false;
            }
        }
    }

    /// Select a game in a direction.
    pub fn select(&mut self, sel_dir: SelDir) {
        use SelDir::{Down, Left, Right, Up};
        let Some(selected) = self.selected else {
            self.selected = match sel_dir {
                Up | Left => self.displayed().next_back(),
                Down | Right => self.displayed().next(),
                SelDir::None => Option::None,
            }
            .map(|game| game.id);
            return;
        };

        let m = |game: &WithThumb| game.id == selected;

        let idx = match sel_dir {
            Up | Left => self.displayed().rev().position(m),
            Down | Right => self.displayed().position(m),
            SelDir::None => Option::None,
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
            SelDir::None => Option::None,
        }
        .map(|game| game.id);
    }

    /// Render elements.
    pub fn view(&self, shadowed: bool) -> Element<'_, OrRequest<Message, Request>> {
        fn card<'a>(
            game: &'a WithThumb,
            width: f32,
            shadowed: bool,
            selected: Option<i64>,
        ) -> Element<'a, OrRequest<Message, Request>> {
            let handle = game.thumb.as_ref();
            let name = game.name.as_str();
            let id = game.id;

            let style: fn(&::iced::Theme) -> container::Style;
            if selected == Some(id) {
                if game.batch_selected {
                    style = |theme| {
                        container::bordered_box(theme)
                            .border(Border {
                                width: 1.0,
                                radius: 0.into(),
                                color: theme.palette().danger,
                            })
                            .background(theme.palette().primary)
                    };
                } else {
                    style =
                        |theme| container::bordered_box(theme).background(theme.palette().primary);
                }
            } else {
                if game.batch_selected {
                    style =
                        |theme| container::bordered_box(theme).background(theme.palette().danger);
                } else {
                    style = container::bordered_box;
                }
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
                    area.on_release(AreaMessage::Select { id })
                        .on_middle_release(AreaMessage::Run { id, sandbox: true })
                        .on_right_release(AreaMessage::BatchSelect { id })
                }
            })
            .pipe(Element::from)
            .map(Into::into)
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

async fn create_cache_dir(cache_path: &Path, tx: StatusSender) -> ControlFlow<()> {
    if let Err(err) = ::tokio::fs::create_dir_all(&cache_path).await {
        ::log::error!("could not create cache directory {cache_path:?}\n{err}");
        status!(tx, "could not create {cache_path:?}");
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
}

fn handle_cache_result(result: Result<Result<(), ::rusqlite::Error>, JoinError>) {
    match result {
        Ok(result) => match result {
            Ok(_) => {}
            Err(err) => ::log::error!("could not cache thumbnails\n{err}"),
        },
        Err(err) => ::log::error!("cache thread failed\n{err}"),
    }
}

fn handle_uncache_result(result: Result<Result<(), ::rusqlite::Error>, JoinError>) {
    match result {
        Ok(result) => match result {
            Ok(_) => {}
            Err(err) => ::log::error!("could not uncache thumbnails\n{err}"),
        },
        Err(err) => ::log::error!("uncache thread failed\n{err}"),
    }
}

async fn uncache_image(slug: String, cache_path: PathBuf) {
    if !::tokio::fs::try_exists(&cache_path).await.unwrap_or(false) {
        return;
    }

    handle_uncache_result(spawn_blocking(move || uncache_image_blocking(slug, cache_path)).await);
}

async fn cache_image(
    slug: String,
    image: ::spel_katalog_formats::Image,
    cache_path: PathBuf,
    tx: StatusSender,
) {
    if create_cache_dir(&cache_path, tx).await.is_break() {
        return;
    }

    handle_cache_result(
        spawn_blocking(move || cache_image_blocking(slug, image, cache_path)).await,
    );
}

async fn cache_images(
    slugs: Vec<String>,
    images: Vec<::spel_katalog_formats::Image>,
    cache_path: PathBuf,
    tx: StatusSender,
    tracker: Option<Tracker>,
) {
    if create_cache_dir(&cache_path, tx).await.is_break() {
        return;
    }

    handle_cache_result(
        spawn_blocking(move || cache_images_blocking(slugs, images, cache_path)).await,
    );

    if let Some(tracker) = tracker {
        tracker.finish();
    }
}

const CREATE_IMAGE_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS images(
    slug TEXT NOT NULL UNIQUE ON CONFLICT REPLACE,
    image BLOB NOT NULL
)
"#;

const INSERT_IMAGE: &str = r#"
INSERT INTO images (slug, image) VALUES (:slug, :image)   
"#;

const REMOVE_IMAGE: &str = r#"
DELETE FROM images WHERE slug = :slug
"#;

fn convert_slug_image(
    slug: String,
    image: ::spel_katalog_formats::Image,
) -> Option<(String, Vec<u8>)> {
    let ::spel_katalog_formats::Image {
        width,
        height,
        bytes,
    } = image;
    let image = ::image::RgbaImage::from_raw(width, height, bytes.into())?;
    let mut buf = Vec::<u8>::new();

    if let Err(err) = image.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png) {
        ::log::error!("failed to convert thumbnail for {slug} to png\n{err}");
        return None;
    };

    Some((slug, buf))
}

fn insert_image(stmt: &mut Statement<'_>, slug: String, image: Vec<u8>) {
    if let Err(err) = stmt.execute(named_params! {":slug": slug, ":image": image}) {
        ::log::error!("failed to save thumbnail for {slug} to cache\n{err}");
    }
}

fn remove_image(stmt: &mut Statement<'_>, slug: String) {
    if let Err(err) = stmt.execute(named_params! {":slug": slug}) {
        ::log::error!("failed to remove thumbnail for {slug} from cache\n{err}");
    }
}

fn uncache_image_blocking(slug: String, cache_path: PathBuf) -> Result<(), ::rusqlite::Error> {
    let cache_path = cache_path.join(THUMBNAILS_FILENAME);
    let db = Connection::open(&cache_path)?;

    let mut stmt = db.prepare_cached(CREATE_IMAGE_TABLE)?;
    stmt.execute([])?;

    let mut stmt = db.prepare_cached(REMOVE_IMAGE)?;
    remove_image(&mut stmt, slug);
    Ok(())
}

fn cache_image_blocking(
    slug: String,
    image: ::spel_katalog_formats::Image,
    cache_path: PathBuf,
) -> Result<(), ::rusqlite::Error> {
    let cache_path = cache_path.join(THUMBNAILS_FILENAME);
    let db = Connection::open(&cache_path)?;

    let mut stmt = db.prepare_cached(CREATE_IMAGE_TABLE)?;
    stmt.execute([])?;

    let mut stmt = db.prepare_cached(INSERT_IMAGE)?;

    if let Some((slug, image)) = convert_slug_image(slug, image) {
        insert_image(&mut stmt, slug, image);
    }

    Ok(())
}

fn cache_images_blocking(
    slugs: Vec<String>,
    images: Vec<::spel_katalog_formats::Image>,
    cache_path: PathBuf,
) -> Result<(), ::rusqlite::Error> {
    let cache_path = cache_path.join(THUMBNAILS_FILENAME);
    let db = Connection::open(&cache_path)?;

    let mut stmt = db.prepare_cached(CREATE_IMAGE_TABLE)?;
    stmt.execute([])?;

    let mut stmt = db.prepare_cached(INSERT_IMAGE)?;

    let mut slugs_images = Vec::new();
    slugs
        .into_iter()
        .zip(images)
        .collect::<Vec<_>>()
        .into_par_iter()
        .map(|(slug, image)| convert_slug_image(slug, image))
        .collect_into_vec(&mut slugs_images);

    for (slug, image) in slugs_images.into_iter().flatten() {
        insert_image(&mut stmt, slug, image);
    }

    Ok(())
}
