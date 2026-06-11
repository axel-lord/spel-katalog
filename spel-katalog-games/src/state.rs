//! [State], [Message] and [Request] impls.

use ::core::{cell::Cell, convert::identity, iter, mem, ops::ControlFlow, time::Duration};
use ::std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use ::derive_more::{Deref, DerefMut, IsVariant};
use ::iced_core::{
    Alignment::{self},
    Border,
    Length::Fill,
    text::Wrapping,
};
use ::iced_futures::Subscription;
use ::iced_runtime::Task;
use ::iced_widget::{self as widget, container, stack};
use ::image::ImageFormat;
use ::itertools::Itertools;
use ::parking_lot::Mutex;
use ::rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use ::rusqlite::{Connection, Statement, named_params};
use ::rustc_hash::FxHashSet;
use ::spel_katalog_common::{IntoOrRequest, OrRequest, StatusSender, async_status, status, w};
use ::spel_katalog_formats::{Game, GameId, NativeGame};
use ::spel_katalog_gather::{
    CoverGatherer, CoverGathererOptions, LoadDbError, load_games_from_database,
    load_thumbnail_database,
};
use ::spel_katalog_settings::{CacheDir, CoverartDir, Settings};
use ::tap::Pipe;
use ::uuid::Uuid;

use crate::{Element, Games, games::WithThumb};

/// Filename of thumbnails cache database.
const THUMBNAILS_FILENAME: &str = "thumbnails.db";

/// State of games element.
#[derive(Debug, Default, Deref, DerefMut)]
pub struct State {
    #[deref]
    #[deref_mut]
    /// Game collection.
    games: Games,
    /// Queue used for batching caching of thumbnails.
    cache_queue: (Vec<String>, Vec<::spel_katalog_formats::Image>),
    /// Indices of currently selected games.
    selected: Option<GameId>,
    /// How many columns to display.
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
#[derive(Debug, IsVariant, Clone)]
pub enum Message {
    /// Load games from local lutris database.
    LoadDb {
        /// Path to database to load.
        db_path: PathBuf,
    },
    /// Add games.
    AddGames {
        /// Games to add.
        games: Vec<Game>,
    },
    /// Add a single game.
    AddNativeGames {
        /// Games to add.
        games: Vec<(Uuid, NativeGame, Option<::spel_katalog_formats::Image>)>,
    },
    /// Add a single game.
    AddNativeGame {
        /// Uuid of game to add.
        uuid: Uuid,
        /// Config of game.
        config: Box<NativeGame>,
        /// Thumbnail of game.
        thumb: Option<::spel_katalog_formats::Image>,
    },
    /// Set thumbnails.
    SetImages {
        /// Slugs for games to set thumbnails for.
        slugs: Vec<String>,
        /// Thumbnails to set.
        images: Vec<::spel_katalog_formats::Image>,
        /// True if images should be added to cache.
        add_to_cache: bool,
    },
    /// Set a single thumbnail.
    SetImage {
        /// Slug.
        slug: String,
        /// Image.
        image: ::spel_katalog_formats::Image,
        /// If true image should be added to cache.
        add_to_cache: bool,
    },
    /// Remove a thumbnail from game and cache.
    RemoveImage {
        /// Game slug.
        slug: String,
    },
    /// Move selection.
    Select(SelDir),
    /// Select an id.
    SelectId(GameId),
    /// Batch select game.
    BatchSelect(GameId),
    /// FLush thumbnail cache to database.
    FlushCache,
    /// Force re-sort of games.
    Sort,
}

/// Requests for other widgets.
#[derive(Debug, IsVariant, Clone)]
pub enum Request {
    /// Set currently chosen game.
    ShowGame {
        /// Id of game
        id: GameId,
    },
    /// Run a game.
    Run {
        /// Id of game.
        id: GameId,
        /// Should the game be sandboxed.
        sandbox: bool,
    },
    /// Close game info
    CloseInfo,
    /// Convert game to native.
    Convert(GameId),
}

/// Messages produced by game areas.
#[derive(Debug, Clone, Copy)]
pub enum AreaMessage {
    /// Select a game.
    Select {
        /// Numeric id of game.
        id: GameId,
    },
    /// Given id was batch selected.
    BatchSelect {
        /// Numeric id game.
        id: GameId,
    },
    /// Run game.
    Run {
        /// Numeric id of game.
        id: GameId,
        /// Should the game be sandboxed.
        sandbox: bool,
    },
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
    pub const fn columns(&self) -> usize {
        self.columns.get()
    }

    /// Get id of currently selected game, if any.
    pub const fn selected(&self) -> Option<GameId> {
        self.selected
    }

    /// Deselect game.
    pub const fn deselect(&mut self) {
        self.selected = None;
    }

    /// Subscription used by games state.
    pub fn subscription(&self) -> Subscription<Message> {
        if !self.cache_queue.0.is_empty() {
            ::iced_futures::backend::default::time::every(Duration::from_secs_f64(0.1))
                .map(|_| Message::FlushCache)
        } else {
            Subscription::none()
        }
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
            Message::Sort => {
                self.sort(settings, filter);
                Task::none()
            }
            Message::LoadDb { db_path } => {
                let tx = tx.clone();
                Task::future(async move {
                    match ::smol::unblock(move || load_games_from_database(&db_path)).await {
                        Ok(games) => games
                            .pipe(|games| Message::AddGames { games })
                            .pipe(OrRequest::Message)
                            .pipe(Task::done),
                        Err(err) => match err {
                            LoadDbError::Sqlite(error) => {
                                ::log::error!("an sqlite error occurred\n{error}");
                                async_status!(tx, "an sqlite error occurred").await;
                                Task::none()
                            }
                        },
                    }
                })
                .then(identity)
            }
            Message::AddGames { games } => {
                self.add_games(
                    games.into_iter().map(|game| WithThumb {
                        game,
                        thumb: None,
                        batch_selected: false,
                        shadows: None,
                        ghost: false,
                    }),
                    settings,
                    filter,
                );

                status!(tx, "read games from database");

                self.find_cached(settings)
            }
            Message::AddNativeGames { games } => {
                self.add_games(
                    games.into_iter().map(|(uuid, game, thumb)| WithThumb {
                        thumb: thumb.map(
                            |::spel_katalog_formats::Image {
                                 width,
                                 height,
                                 bytes,
                             }| {
                                ::iced_core::image::Handle::from_rgba(width, height, bytes)
                            },
                        ),
                        ..WithThumb::from((uuid, game))
                    }),
                    settings,
                    filter,
                );
                Task::none()
            }
            Message::AddNativeGame {
                uuid,
                config,
                thumb,
            } => {
                self.add_games(
                    iter::once(WithThumb {
                        thumb: thumb.map(
                            |::spel_katalog_formats::Image {
                                 width,
                                 height,
                                 bytes,
                             }| {
                                ::iced_core::image::Handle::from_rgba(width, height, bytes)
                            },
                        ),
                        ..WithThumb::from((uuid, *config))
                    }),
                    settings,
                    filter,
                );
                Task::none()
            }
            Message::SetImages {
                slugs,
                images,
                add_to_cache,
            } => {
                for (slug, image) in slugs.iter().zip(&images) {
                    self.set_image(slug, image.clone());
                }

                if add_to_cache {
                    let (slug_queue, image_queue) = &mut self.cache_queue;
                    slug_queue.extend(slugs);
                    image_queue.extend(images);
                }

                Task::none()
            }
            Message::SetImage {
                slug,
                image,
                add_to_cache,
            } => {
                self.set_image(&slug, image.clone());

                if add_to_cache {
                    let (slugs, images) = &mut self.cache_queue;
                    slugs.push(slug);
                    images.push(image);
                }

                Task::none()
            }
            Message::FlushCache => {
                let (slugs, images) = mem::take(&mut self.cache_queue);
                let cache_path = settings.get::<CacheDir>().to_path_buf();
                Task::future(cache_images(slugs, images, cache_path, tx.clone()))
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
                Task::done(OrRequest::Request(Request::ShowGame { id }))
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
    pub fn find_cached(&mut self, settings: &Settings) -> Task<OrRequest<Message, Request>> {
        let cache_dir = settings.get::<CacheDir>().to_path_buf();
        let cover_dir = settings.get::<CoverartDir>().to_path_buf();

        let game_slugs = self
            .all()
            .iter()
            .filter_map(|game| game.slug().map(ToOwned::to_owned))
            .collect::<Vec<_>>();

        let find_cached = ::smol::unblock(move || {
            let db_path = cache_dir.join(THUMBNAILS_FILENAME);
            let (slugs, images) = load_thumbnail_database(&db_path)
                .map_err(|err| ::log::warn!("could not load thumbnail cache at {db_path:?}\n{err}"))
                .unwrap_or_default()
                .into_iter()
                .unzip();
            let mut game_slugs = FxHashSet::from_iter(game_slugs);

            for slug in &slugs {
                game_slugs.remove(slug);
            }

            let game_slugs = Vec::from_iter(game_slugs);

            (game_slugs, slugs, images)
        });

        Task::future(find_cached).then(move |(game_slugs, slugs, images)| {
            let set_images = Message::SetImages {
                slugs,
                images,
                add_to_cache: false,
            }
            .pipe(OrRequest::Message)
            .pipe(Task::done);

            let load_covers = if !game_slugs.is_empty() {
                ::log::info!(
                    "looking for {} thumbnails in {cover_dir:?}",
                    game_slugs.len()
                );
                CoverGatherer::with_options(
                    &cover_dir,
                    CoverGathererOptions {
                        slugs: Some(game_slugs),
                        ..Default::default()
                    },
                )
                .map_err(|err| ::log::warn!("could not read cover dir {cover_dir:?}\n{err}"))
                .map(|cover_gatherer| cover_gatherer.into_stream().pipe(Task::stream))
                .ok()
                .unwrap_or_else(Task::none)
                .map(|(slug, image)| Message::SetImage {
                    slug,
                    image,
                    add_to_cache: true,
                })
                .map(OrRequest::Message)
            } else {
                ::log::info!("no need to load covers from {cover_dir:?}");
                Task::none()
            };

            Task::batch([set_images, load_covers])
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
            .map(|game| game.id());
            return;
        };

        let m = |game: &WithThumb| game.id() == selected;

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
            Up => self.displayed().rev().cycle().nth(idx + self.columns()),
            Down => self.displayed().cycle().nth(idx + self.columns()),
            Left => self.displayed().rev().cycle().nth(idx + 1),
            Right => self.displayed().cycle().nth(idx + 1),
            SelDir::None => Option::None,
        }
        .map(|game| game.id());
    }

    /// Render elements.
    pub fn view(&self) -> Element<'_, OrRequest<Message, Request>> {
        fn card<'a>(
            game: &'a WithThumb,
            width: f32,
            selected: Option<GameId>,
        ) -> Element<'a, OrRequest<Message, Request>> {
            let handle = game.thumb.as_ref();
            let name = game.name();
            let id = game.id();

            fn base(theme: &::iced_core::Theme) -> container::Style {
                let style = container::bordered_box(theme);
                style.border(Border {
                    radius: 0.into(),
                    ..style.border
                })
            }

            fn batch_and_select(theme: &::iced_core::Theme) -> container::Style {
                let style = select(theme);
                style.border(style.border.color(theme.palette().danger))
            }

            fn select(theme: &::iced_core::Theme) -> container::Style {
                base(theme).background(theme.palette().primary.scale_alpha(0.9))
            }

            fn batch(theme: &::iced_core::Theme) -> container::Style {
                base(theme).background(theme.palette().danger.scale_alpha(0.9))
            }

            fn not_selected(theme: &::iced_core::Theme) -> container::Style {
                base(theme).background(theme.palette().background.scale_alpha(0.95))
            }

            let style: fn(&::iced_core::Theme) -> container::Style =
                match (selected, game.batch_selected) {
                    (Some(id), false) if game.id() == id => select,
                    (Some(id), true) if game.id() == id => batch_and_select,
                    (_, true) => batch,
                    (_, false) => not_selected,
                };

            let text = widget::text(name)
                .wrapping(Wrapping::WordOrGlyph)
                .size(14)
                .pipe(container)
                .padding(3)
                .style(style)
                .pipe(container)
                .width(width)
                .height(width)
                .padding(3)
                .align_x(Alignment::Center)
                .align_y(Alignment::End);

            let element = match handle {
                Some(handle) => {
                    let image = widget::image(handle)
                        .width(width)
                        .height(width)
                        .content_fit(::iced_core::ContentFit::Contain);
                    widget::mouse_area(stack([image.into(), text.into()]))
                }
                None => widget::mouse_area(text),
            }
            .interaction(::iced_core::mouse::Interaction::Pointer)
            .on_release(AreaMessage::Select { id })
            .on_middle_release(AreaMessage::Run { id, sandbox: true })
            // .on_right_release(AreaMessage::BatchSelect { id })
            .pipe(Element::from)
            .map(OrRequest::<Message, Request>::from);

            ::iced_aw::ContextMenu::new(element, move || {
                ::spel_katalog_widget::ListMenu::new()
                    .push(widget::text("Game"))
                    .separator()
                    .button("Run", move || {
                        Request::Run { id, sandbox: true }.into_request()
                    })
                    .button("Batch", move || Message::BatchSelect(id).into_message())
                    .button("Info", move || Message::SelectId(id).into_message())
                    .separator()
                    .button("Convert", move || Request::Convert(id).into_request())
                    .into()
            })
            .into()
        }

        widget::responsive(move |size| {
            let columns = ((size.width as usize - 1) / 153).clamp(1, 24);
            let width = ((size.width / columns as f32) - 3.0).clamp(150.0, 300.0);
            self.columns.set(columns);

            spel_katalog_widget::scrollable(w::col().align_x(Alignment::Start).width(Fill).extend(
                self.displayed().chunks(columns).into_iter().map(|chunk| {
                    w::row()
                        .extend(chunk.map(|game| card(game, width, self.selected)))
                        .into()
                }),
            ))
            .id(widget::Id::new("games-view"))
            .into()
        })
        .pipe(Element::from)
    }
}

/// Create cache directory.
async fn create_cache_dir(cache_path: &Path, tx: StatusSender) -> ControlFlow<()> {
    if let Err(err) = ::smol::fs::create_dir_all(&cache_path).await {
        ::log::error!("could not create cache directory {cache_path:?}\n{err}");
        status!(tx, "could not create {cache_path:?}");
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
}

/// Remove an image from cache.
async fn uncache_image(slug: String, cache_path: PathBuf) {
    let result = ::smol::unblock(move || uncache_image_blocking(slug, cache_path)).await;
    match result {
        Ok(_) => {}
        Err(err) => ::log::error!("could not uncache thumbnails\n{err}"),
    }
}

/// Add an image to cache.
async fn cache_images(
    slugs: Vec<String>,
    images: Vec<::spel_katalog_formats::Image>,
    cache_path: PathBuf,
    tx: StatusSender,
) {
    if create_cache_dir(&cache_path, tx).await.is_break() {
        return;
    }

    let result = ::smol::unblock(move || cache_images_blocking(slugs, images, cache_path)).await;
    match result {
        Ok(_) => {}
        Err(err) => ::log::error!("could not cache thumbnails\n{err}"),
    }
}

/// SQL to create image table.
const CREATE_IMAGE_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS images(
    slug TEXT NOT NULL UNIQUE ON CONFLICT REPLACE,
    image BLOB NOT NULL
)
"#;

/// SQL to insert image into table.
const INSERT_IMAGE: &str = r#"
INSERT INTO images (slug, image) VALUES (:slug, :image)   
"#;

/// SQL to remove image from table.
const REMOVE_IMAGE: &str = r#"
DELETE FROM images WHERE slug = :slug
"#;

/// Convert image to png.
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

/// Insert image into database.
fn insert_image(stmt: &mut Statement<'_>, slug: String, image: Vec<u8>) {
    if let Err(err) = stmt.execute(named_params! {":slug": slug, ":image": image}) {
        ::log::error!("failed to save thumbnail for {slug} to cache\n{err}");
    }
}

/// Remove image from database.
fn remove_image(stmt: &mut Statement<'_>, slug: String) {
    if let Err(err) = stmt.execute(named_params! {":slug": slug}) {
        ::log::error!("failed to remove thumbnail for {slug} from cache\n{err}");
    }
}

/// Blocking portion of image uncaching.
fn uncache_image_blocking(slug: String, cache_path: PathBuf) -> Result<(), ::rusqlite::Error> {
    let cache_path = cache_path.join(THUMBNAILS_FILENAME);
    let db = Connection::open(&cache_path)?;

    let mut stmt = db.prepare_cached(CREATE_IMAGE_TABLE)?;
    stmt.execute([])?;

    let mut stmt = db.prepare_cached(REMOVE_IMAGE)?;
    remove_image(&mut stmt, slug);
    Ok(())
}

/// Blocking portion of image caching.
fn cache_images_blocking(
    slugs: Vec<String>,
    images: Vec<::spel_katalog_formats::Image>,
    cache_path: PathBuf,
) -> Result<(), ::rusqlite::Error> {
    static LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
    let lock = &*LOCK;
    let lock = lock.lock();

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

    drop(lock);
    Ok(())
}
