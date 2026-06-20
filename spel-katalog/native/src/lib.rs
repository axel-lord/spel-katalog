//! Utilities for working with native games.

use ::core::{fmt::Display, time::Duration};
use ::std::{
    borrow::Cow,
    io::{Cursor, Read},
    path::Path,
};

use ::bytemuck::TransparentWrapper;
use ::bytes::{BufMut, BytesMut};
use ::derive_more::{AsMut, AsRef, Deref, DerefMut, From, Into, IsVariant};
use ::flate2::{
    Compression,
    bufread::{GzDecoder, GzEncoder},
};
use ::image::{
    DynamicImage, EncodableLayout, GenericImage, ImageFormat, Rgba, RgbaImage,
    imageops::FilterType::Lanczos3,
};
use ::r2d2::PooledConnection;
use ::r2d2_sqlite::SqliteConnectionManager;
use ::rusqlite::CachedStatement;
use ::spel_katalog_formats::NativeGame;
use ::uuid::Uuid;

/// Log error and return/execute statement if value is an error.
macro_rules! log_err {
    ($value:expr, $err:ident, $fmt:literal $(, $arg:expr)* $(,)?) => {
        log_err!($value, ::log::Level::Error, $err, $fmt $(, $arg)*)
    };
    ($value:expr, $level:expr, $err:ident, $fmt:literal $(, $arg:expr)* $(,)?) => {
        log_err!($value, $level, $err, ($fmt $(, $arg)*))

    };
    ($value:expr, $err:ident, ($fmt:literal $(, $arg:expr)* $(,)?) $(, $stmt:stmt)?) => {
        log_err!($value, ::log::Level::Error, $err, ($fmt $(, $arg)*) $(, $stmt)*)
    };
    ($value:expr, $level:expr, $err:ident, ($fmt:literal $(, $arg:expr)* $(,)?) $(, $stmt:stmt)?) => {
        match $value {
            Ok(v) => v,
            Err($err) => {
                {
                    #![allow(unused, reason = "some macro invocations may leave return unused")]
                    ::log::log!($level, $fmt $(, $arg)*);
                    $($stmt)*
                    return;
                }
            }
        }
    };
}

/// Size of thumbnails.
pub const THUMBNAIL_SIZE: u32 = 200;

/// Generate a thumbnail for the given image.
pub fn thumbnail(image: DynamicImage) -> ::spel_katalog_formats::Image {
    let thumb = if image.height() > THUMBNAIL_SIZE || image.width() > THUMBNAIL_SIZE {
        image.resize(THUMBNAIL_SIZE, THUMBNAIL_SIZE, Lanczos3)
    } else {
        image
    }
    .into_rgba8();

    ::spel_katalog_formats::Image {
        width: thumb.width(),
        height: thumb.height(),
        bytes: thumb.into_raw().into(),
    }
}

/// Make a thumbnail square by letterboxing with average color.
pub fn make_square_thumbnail(image: Cow<'_, DynamicImage>) -> Option<Cow<'_, DynamicImage>> {
    if image.width() != image.height() {
        let single = image
            .resize_exact(1, 1, ::image::imageops::FilterType::Lanczos3)
            .into_rgba8();
        let [r, g, b, _] = single.get_pixel(0, 0).0;
        let dim = image.width().max(image.height());
        let mut canvas = RgbaImage::from_pixel(dim, dim, Rgba([r, g, b, 192]));
        canvas
            .copy_from(
                &*image,
                dim.checked_sub(image.width())? / 2,
                dim.checked_sub(image.height())? / 2,
            )
            .map_err(|err| ::log::error!("failed to format thumbnail\n{err}"))
            .ok()?;
        Some(Cow::Owned(canvas.into()))
    } else {
        Some(image)
    }
}

/// Connection pool used to read games.
#[derive(Debug, Clone, TransparentWrapper, Deref, DerefMut, AsMut, AsRef, From, Into)]
#[repr(transparent)]
pub struct Pool {
    /// Wrapped r2d2 pool.
    inner: ::r2d2::Pool<SqliteConnectionManager>,
}

/// Error returned when [Pool::new] fails.
#[derive(Debug, ::thiserror::Error)]
#[error(transparent)]
pub struct PoolCreationError {
    /// Wrapped error.
    #[from]
    inner: ::r2d2::Error,
}

/// Database errors common between interactions.
#[derive(Debug, ::thiserror::Error, IsVariant)]
pub enum DbError {
    /// Database connection could not be grabbed.
    #[error("could not grab database connection, {0}")]
    GetConnection(::r2d2::Error),
    /// Replace statement could not be prepared.
    #[error("could not prepare statement, {0}")]
    PrepareStatement(::rusqlite::Error),
}

/// Error returned when [Pool::insert_game] fails to insert game config.
#[derive(Debug, ::thiserror::Error, IsVariant)]
pub enum InsertGameError {
    /// Statement preparation or database connection failed.
    #[error(transparent)]
    Database(#[from] DbError),
    /// Replace statement failed.
    #[error("could not replace game using statement, {0}")]
    ReplaceGame(::rusqlite::Error),
    /// Game config could not be serialized as toml.
    #[error("could not serialize game config, {0}")]
    SerializeConfig(::toml::ser::Error),
    /// Game config could not be compressed.
    #[error("could not compress game config, {0}")]
    CompressConfig(::std::io::Error),
}

/// Error returned when [Pool::insert_thumb] fails to insert game config.
#[derive(Debug, ::thiserror::Error, IsVariant)]
pub enum InsertThumbError {
    /// Statement preparation or database connection failed.
    #[error(transparent)]
    Database(#[from] DbError),
    /// Replace statement failed.
    #[error("could not replace thumbnail using statement, {0}")]
    ReplaceThumbnail(::rusqlite::Error),
    /// Thumbnail could not be encoded.
    #[error("could not encode thumbnail, {0}")]
    EncodeImage(::image::ImageError),
}

/// Error returned when [Pool::get_game] fails to grab game.
#[derive(Debug, ::thiserror::Error, IsVariant)]
pub enum GetError {
    /// Statement preparation or database connection failed.
    #[error(transparent)]
    Database(#[from] DbError),
    /// No games found matching the uuid.
    #[error("query failed, {0}")]
    NoResults(::rusqlite::Error),
    /// Config could not be decompressed.
    #[error("could not decompress config")]
    DecompressConfig(::std::io::Error),
    /// Could not deserialize game data.
    #[error("could not deserialize game data, {0}")]
    DeserializeConfig(::toml::de::Error),
    /// Could not decode image.
    #[error("could not decode image, {0}")]
    DecodeImage(::image::ImageError),
}

/// Error returned when [Pool::remove_thumb] fails to remove a thumbnail.
#[derive(Debug, ::thiserror::Error, IsVariant)]
pub enum RemoveThumbError {
    /// Statement preparation or database connection failed.
    #[error(transparent)]
    Database(#[from] DbError),
    /// Thumbnail could not be removed.
    #[error("could not remove thumbnail\n{0}")]
    RemoveThumbnail(::rusqlite::Error),
}

mod builder {
    #![allow(clippy::missing_docs_in_private_items)]

    use ::image::DynamicImage;
    use ::spel_katalog_formats::NativeGame;
    use ::uuid::Uuid;

    use crate::{DbError, InsertGameError, InsertThumbError, Pool};

    #[bon::bon]
    impl Pool {
        /// Insert a game or update it's contents.
        ///
        /// # Errors
        /// If the game cannot be inserted. Failure to
        /// insert thumbnail will only be logged.
        #[builder(finish_fn = insert)]
        pub fn insert_game(
            &self,
            /// Uuid of game to insert.
            #[builder(start_fn)]
            uuid: Uuid,
            /// Game config to insert.
            #[builder(finish_fn)]
            config: &NativeGame,
            /// Thumbnail to insert for game.
            thumb: Option<&DynamicImage>,
            /// Reuse buffer.
            buf: Option<&mut Vec<u8>>,
        ) -> Result<(), InsertGameError> {
            let conn = self.get().map_err(DbError::GetConnection)?;
            let mut backing;
            let buf = if let Some(buf) = buf {
                buf
            } else {
                backing = Vec::new();
                &mut backing
            };
            Self::insert_game_(&conn, uuid, config, thumb, buf)
        }

        /// Insert a thumbnail or replace it.
        ///
        /// # Errors
        /// If thethumbnail cannot be inserted.
        #[builder(finish_fn = insert)]
        pub fn insert_thumb(
            &self,
            /// Uuid of game to insert thumbnail for.
            #[builder(start_fn)]
            uuid: Uuid,
            /// Thumbnail to insert for game.
            #[builder(finish_fn)]
            thumb: &DynamicImage,
            /// Reuse buffer.
            buf: Option<&mut Vec<u8>>,
        ) -> Result<(), InsertThumbError> {
            let conn = self.get().map_err(DbError::GetConnection)?;
            let mut backing;
            let buf = if let Some(buf) = buf {
                buf
            } else {
                backing = Vec::new();
                &mut backing
            };
            Self::insert_thumb_(&conn, uuid, thumb, buf)
        }
    }
}

impl Pool {
    /// Get a connection to the database.
    fn get_conn(&self) -> Result<PooledConnection<SqliteConnectionManager>, DbError> {
        self.get().map_err(DbError::GetConnection)
    }

    /// Prepare a statement on connectio.
    fn prep_stmt<'a>(
        conn: &'a ::rusqlite::Connection,
        sql: &str,
    ) -> Result<CachedStatement<'a>, DbError> {
        conn.prepare_cached(sql).map_err(DbError::PrepareStatement)
    }

    /// Construct a new connection pool.
    ///
    /// # Errors
    /// If the pool cannot be created.
    pub fn new(database: &Path) -> Result<Self, PoolCreationError> {
        const INIT_DB: &str = r"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS games (
                uuid BLOB NOT NULL PRIMARY KEY,
                config BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS thumbs (
                uuid BLOB NOT NULL PRIMARY KEY,
                image BLOB NOT NULL,
                FOREIGN KEY (uuid)
                    REFERENCES games (uuid)
                        ON UPDATE CASCADE
                        ON DELETE CASCADE
            );
        ";
        let manager = SqliteConnectionManager::file(database).with_init(|c| {
            c.execute_batch(INIT_DB)
                .inspect_err(|err| ::log::error!("db init failed, {err}"))
        });
        let inner = ::r2d2::Pool::builder()
            .max_size(1)
            .min_idle(Some(0))
            .idle_timeout(Some(Duration::from_mins(5)))
            .build(manager)?;

        Ok(Self { inner })
    }

    /// Get a game from database.
    ///
    /// # Errors
    /// If the game cannot be retrieved.
    pub fn get_game(&self, game_id: Uuid) -> Result<NativeGame, GetError> {
        const SELECT_GAME: &str = r"
            SELECT config FROM games
            WHERE uuid = $1
        ";
        let conn = self.get_conn()?;
        let mut stmt = Self::prep_stmt(&conn, SELECT_GAME)?;

        let decoded = stmt
            .query_one((game_id,), |row| {
                let bytes = row.get_ref(0)?.as_bytes()?;
                let mut buf = BytesMut::with_capacity(bytes.len()).writer();
                let mut config_reader = GzDecoder::new(bytes);
                let result = ::std::io::copy(&mut config_reader, &mut buf);
                let buf = buf.into_inner();
                Ok(result.map(move |_| buf))
            })
            .map_err(GetError::NoResults)?
            .map_err(GetError::DecompressConfig)?;
        let parsed =
            ::toml::from_slice::<NativeGame>(&decoded).map_err(GetError::DeserializeConfig)?;
        Ok(parsed)
    }

    /// Remove game from database.
    ///
    /// # Errors
    /// If the statement cannot be prepared,
    /// or if no database connectetion is established.
    ///
    /// Will not error if the game does not exist. Result will
    /// however be logged.
    pub fn remove_game(&self, game_id: Uuid) -> Result<(), DbError> {
        const DELETE_THUMB: &str = r"
            DELETE FROM games
            WHERE uuid = $1
        ";

        let conn = self.get_conn()?;
        let mut stmt = Self::prep_stmt(&conn, DELETE_THUMB)?;

        match stmt.execute((game_id,)) {
            Ok(0) => {
                ::log::warn!("game with uuid {game_id} not deleted");
            }
            Ok(n) => {
                ::log::info!("deleted {n} games with uuid {game_id}");
            }
            Err(err) => {
                ::log::error!("failed to delete game with id {game_id}\n{err}");
            }
        }

        Ok(())
    }

    /// Get a thumbnail from database.
    ///
    /// # Errors
    /// If the thumbnail cannot be retrieved.
    pub fn get_thumb(&self, game_id: Uuid) -> Result<DynamicImage, GetError> {
        const SELECT_GAME: &str = r"
            SELECT image FROM thumbs
            WHERE uuid = $1
        ";
        let conn = self.get_conn()?;

        let mut stmt = Self::prep_stmt(&conn, SELECT_GAME)?;

        stmt.query_one((game_id,), |row| {
            let bytes = row.get_ref(0)?.as_bytes()?;
            Ok(::image::load_from_memory_with_format(
                bytes,
                ImageFormat::Png,
            ))
        })
        .map_err(GetError::NoResults)?
        .map_err(GetError::DecodeImage)
    }

    /// Remove a thumbnail from database.
    ///
    /// # Errors
    /// If the statement cannot be prepared,
    /// or if no database connectetion is established.
    ///
    /// Will not error if the thumbnail does not exist. Result will
    /// however be logged.
    pub fn remove_thumb(&self, game_id: Uuid) -> Result<(), DbError> {
        const DELETE_THUMB: &str = r"
            DELETE FROM thumbs
            WHERE uuid = $1
        ";
        let conn = self.get_conn()?;
        let mut stmt = Self::prep_stmt(&conn, DELETE_THUMB)?;

        match stmt.execute((game_id,)) {
            Ok(0) => {
                ::log::warn!("no thumbnails for games with uuid {game_id} deleted");
            }
            Ok(n) => {
                ::log::info!("deleted {n} thumbnails for {game_id}");
            }
            Err(err) => {
                ::log::error!("failed to delete thumbnails for {game_id}\n{err}");
            }
        }

        Ok(())
    }

    /// Insert a thumbnail or replace it.
    ///
    /// # Errors
    /// If thethumbnail cannot be inserted.
    fn insert_thumb_(
        conn: &::rusqlite::Connection,
        uuid: Uuid,
        thumb: &DynamicImage,
        buf: &mut Vec<u8>,
    ) -> Result<(), InsertThumbError> {
        const INSERT_THUMB: &str = r"
            INSERT INTO thumbs (uuid, image)
                VALUES ($1, $2)
                ON CONFLICT(uuid)
                    DO UPDATE SET image=excluded.image
        ";
        buf.clear();
        let mut stmt = Self::prep_stmt(conn, INSERT_THUMB)?;

        {
            thumb
                .write_to(Cursor::new(&mut *buf), ImageFormat::Png)
                .map_err(InsertThumbError::EncodeImage)?;
        }

        stmt.execute((uuid, buf.as_bytes()))
            .map_err(InsertThumbError::ReplaceThumbnail)?;

        Ok(())
    }

    /// Insert a game or update it's contents.
    ///
    /// # Errors
    /// If the game cannot be inserted. Failure to
    /// insert thumbnail will only be logged.
    fn insert_game_(
        conn: &::rusqlite::Connection,
        uuid: Uuid,
        config: &NativeGame,
        thumb: Option<&DynamicImage>,
        buf: &mut Vec<u8>,
    ) -> Result<(), InsertGameError> {
        const INSERT_GAME: &str = r"
            INSERT INTO games (uuid, config)
                VALUES ($1, $2)
                ON CONFLICT(uuid)
                    DO UPDATE SET config=excluded.config
        ";

        buf.clear();
        let mut stmt = Self::prep_stmt(conn, INSERT_GAME)?;

        let config_string = ::toml::to_string(config).map_err(InsertGameError::SerializeConfig)?;
        let mut config_writer = GzEncoder::new(config_string.as_bytes(), Compression::best());

        config_writer
            .read_to_end(buf)
            .map_err(InsertGameError::CompressConfig)?;

        stmt.execute((uuid, buf.as_bytes()))
            .map_err(InsertGameError::ReplaceGame)?;

        if let Some(thumb) = thumb
            && let Err(err) = Self::insert_thumb_(conn, uuid, thumb, buf)
        {
            ::log::error!(
                "failed to insert thumbnail for {uuid}, {title}\n{err}",
                title = config.name
            );
        }

        Ok(())
    }

    /// Collect native games from database.
    pub fn gather(self, for_each: &mut dyn FnMut(Uuid, NativeGame)) {
        const SELECT_GAMES: &str = r"
            SELECT 
                uuid, config
            FROM 
                games
        ";

        /// Get column as blob.
        fn get_column_blob<'a>(
            row: &'a ::rusqlite::Row<'a>,
            idx: usize,
            name: &str,
            row_id: &impl Display,
        ) -> Option<&'a [u8]> {
            match row.get_ref(idx) {
                Ok(column) => match column.as_bytes_or_null() {
                    Ok(blob) => blob,
                    Err(err) => {
                        ::log::error!("could not get {name} column as blob for {row_id}\n{err}");
                        None
                    }
                },
                Err(err) => {
                    ::log::error!("could not get {name} column for {row_id}\n{err}");
                    None
                }
            }
        }

        let conn = log_err!(self.get(), err, "could not grab database connection\n{err}");
        let mut stmt = log_err!(
            conn.prepare_cached(SELECT_GAMES),
            err,
            "failed to prepare query statement\n{err}"
        );
        let mut rows = log_err!(stmt.query([]), err, "could not query games\n{err}");

        let mut buf = Vec::<u8>::new();
        while let Some(row) = rows
            .next()
            .map_err(|err| ::log::error!("failed to get next row of query\n{err}"))
            .ok()
            .flatten()
        {
            let uuid = log_err!(
                row.get::<_, Uuid>(0),
                err,
                ("could not get uuid of\nrow: {row:#?}\n{err}"),
                continue
            );

            let Some(config) = get_column_blob(row, 1, "config", &uuid) else {
                continue;
            };

            buf.clear();
            let mut config_reader = GzDecoder::new(config);

            if let Err(err) = config_reader.read_to_end(&mut buf) {
                ::log::error!("could not decompress config for {uuid}\n{err}");
                continue;
            }

            let config = log_err!(
                ::toml::from_slice::<NativeGame>(&buf),
                err,
                ("could not parse game config of {uuid}\n{err}"),
                continue
            );

            for_each(uuid, config);
        }
    }

    /// Convert an r2d2 pool reference.
    pub fn from_ref(pool: &::r2d2::Pool<SqliteConnectionManager>) -> &Self {
        TransparentWrapper::wrap_ref(pool)
    }
}
