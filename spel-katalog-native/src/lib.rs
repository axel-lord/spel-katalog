//! Utilities for working with native games.

use ::core::{fmt::Display, time::Duration};
use ::std::{
    io::{Cursor, Read},
    path::Path,
};

use ::bytemuck::TransparentWrapper;
use ::derive_more::{AsMut, AsRef, Deref, DerefMut, From, Into, IsVariant};
use ::flate2::{
    Compression,
    bufread::{GzDecoder, GzEncoder},
};
use ::image::{DynamicImage, EncodableLayout, ImageFormat};
use ::r2d2_sqlite::SqliteConnectionManager;
use ::spel_katalog_formats::NativeGame;
use ::uuid::Uuid;

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

/// Error returned when [Pool::collect] fails
#[derive(Debug, ::thiserror::Error, IsVariant)]
pub enum GameCollectionError {
    /// Database connection could not be grabbed.
    #[error("could not grab database connection, {0}")]
    GetConn(::r2d2::Error),
    /// Collection statement could not be prepared.
    #[error("could not prepare select statement, {0}")]
    PrepStmt(::rusqlite::Error),
    /// Games could not be queried with statement.
    #[error("could query games, {0}")]
    QueryGames(::rusqlite::Error),
    /// Next row could not be gotten.
    #[error("error retreiving next row, {0}")]
    NextRow(::rusqlite::Error),
}

/// Error returned when [Pool::insert_game] fails to insert game config.
#[derive(Debug, ::thiserror::Error, IsVariant)]
pub enum InsertGameError {
    /// Database connection could not be grabbed.
    #[error("could not grab database connection, {0}")]
    GetConn(::r2d2::Error),
    /// Replace statement could not be prepared.
    #[error("could not prepare replace statement, {0}")]
    PrepStmt(::rusqlite::Error),
    /// Replace statement failed.
    #[error("could not replace game using statement, {0}")]
    ReplaceGame(::rusqlite::Error),
    /// Game config could not be serialized.
    #[error("could not serialize game config, {0}")]
    WriteToml(::toml::ser::Error),
    /// Game config could not be compressed.
    #[error("could not compress game config, {0}")]
    CompressConf(::std::io::Error),
}

/// Error returned when [Pool::insert_thumb] fails to insert game config.
#[derive(Debug, ::thiserror::Error, IsVariant)]
pub enum InsertThumbError {
    /// Database connection could not be grabbed.
    #[error("could not grab database connection, {0}")]
    GetConn(::r2d2::Error),
    /// Replace statement could not be prepared.
    #[error("could not prepare replace statement, {0}")]
    PrepStmt(::rusqlite::Error),
    /// Replace statement failed.
    #[error("could not replace game using statement, {0}")]
    ReplaceThumb(::rusqlite::Error),
    /// Thumbnail could not be encoded.
    #[error("could not encode thumbnail, {0}")]
    EncodeImage(::image::ImageError),
}

/// Error returned when [Pool::get_game] fails to grab game.
#[derive(Debug, ::thiserror::Error, IsVariant)]
pub enum GetError {
    /// Database connection could not be grabbed.
    #[error("could not grab database connection, {0}")]
    GetConn(::r2d2::Error),
    /// Select statement could not be prepared.
    #[error("could not prepare select statement, {0}")]
    PrepStmt(::rusqlite::Error),
    /// No games found matching the uuid.
    #[error("query failed, {0}")]
    NoResults(::rusqlite::Error),
    /// Data could not be decompressed.
    #[error("could not decompress data")]
    Decompress(::std::io::Error),
    /// Could not deserialize game data.
    #[error("could not deserialize game data, {0}")]
    Parse(::toml::de::Error),
    /// Could not decode image.
    #[error("could not decode image, {0}")]
    DecodeImg(::image::ImageError),
}

mod builder {
    #![allow(clippy::missing_docs_in_private_items)]

    use ::image::DynamicImage;
    use ::spel_katalog_formats::NativeGame;
    use ::uuid::Uuid;

    use crate::{InsertGameError, InsertThumbError, Pool};

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
            thumb: Option<DynamicImage>,
            /// Reuse buffer.
            buf: Option<&mut Vec<u8>>,
        ) -> Result<(), InsertGameError> {
            let conn = self.get().map_err(InsertGameError::GetConn)?;
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
            thumb: DynamicImage,
            /// Reuse buffer.
            buf: Option<&mut Vec<u8>>,
        ) -> Result<(), InsertThumbError> {
            let conn = self.get().map_err(InsertThumbError::GetConn)?;
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
        let conn = self.get().map_err(GetError::GetConn)?;
        let mut stmt = conn
            .prepare_cached(SELECT_GAME)
            .map_err(GetError::PrepStmt)?;

        let decoded = stmt
            .query_one((game_id,), |row| {
                let bytes = row.get_ref(0)?.as_bytes()?;
                let mut buf = Vec::new();
                let mut config_reader = GzDecoder::new(bytes);
                let result = config_reader.read_to_end(&mut buf);
                Ok(result.map(move |_| buf))
            })
            .map_err(GetError::NoResults)?
            .map_err(GetError::Decompress)?;
        ::toml::from_slice::<NativeGame>(&decoded).map_err(GetError::Parse)
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
        let conn = self.get().map_err(GetError::GetConn)?;
        let mut stmt = conn
            .prepare_cached(SELECT_GAME)
            .map_err(GetError::PrepStmt)?;

        stmt.query_one((game_id,), |row| {
            let bytes = row.get_ref(0)?.as_bytes()?;
            Ok(::image::load_from_memory_with_format(
                bytes,
                ImageFormat::Png,
            ))
        })
        .map_err(GetError::NoResults)?
        .map_err(GetError::DecodeImg)
    }

    /// Insert a thumbnail or replace it.
    ///
    /// # Errors
    /// If thethumbnail cannot be inserted.
    fn insert_thumb_(
        conn: &::rusqlite::Connection,
        uuid: Uuid,
        thumb: DynamicImage,
        buf: &mut Vec<u8>,
    ) -> Result<(), InsertThumbError> {
        const INSERT_THUMB: &str = r"
            REPLACE INTO thumbs (uuid, image)
            VALUES ($1, $2)
        ";
        buf.clear();
        let mut stmt = conn
            .prepare_cached(INSERT_THUMB)
            .map_err(InsertThumbError::PrepStmt)?;

        {
            thumb
                .write_to(Cursor::new(&mut *buf), ImageFormat::Png)
                .map_err(InsertThumbError::EncodeImage)?;
        }

        stmt.execute((uuid, buf.as_bytes()))
            .map_err(InsertThumbError::ReplaceThumb)?;

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
        thumb: Option<DynamicImage>,
        buf: &mut Vec<u8>,
    ) -> Result<(), InsertGameError> {
        const INSERT_GAME: &str = r"
            REPLACE INTO games (uuid, config)
            VALUES ($1, $2)
        ";

        buf.clear();
        let mut stmt = conn
            .prepare_cached(INSERT_GAME)
            .map_err(InsertGameError::PrepStmt)?;

        let config_string = ::toml::to_string(config).map_err(InsertGameError::WriteToml)?;
        let mut config_writer = GzEncoder::new(config_string.as_bytes(), Compression::best());

        config_writer
            .read_to_end(buf)
            .map_err(InsertGameError::CompressConf)?;

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
    ///
    /// # Errors
    /// If database cannot be properly communicated with.
    pub fn collect(
        &self,
    ) -> Result<Vec<(Uuid, NativeGame, Option<DynamicImage>)>, GameCollectionError> {
        const SELECT_GAMES: &str = r"
            SELECT 
                games.uuid, config, image 
            FROM 
                games
                LEFT JOIN thumbs USING (uuid)
        ";
        let mut games = Vec::new();
        let conn = self.get().map_err(GameCollectionError::GetConn)?;
        let mut stmt = conn
            .prepare_cached(SELECT_GAMES)
            .map_err(GameCollectionError::PrepStmt)?;

        let mut rows = stmt.query([]).map_err(GameCollectionError::QueryGames)?;

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

        let mut buf = Vec::<u8>::new();
        while let Some(row) = rows.next().map_err(GameCollectionError::NextRow)? {
            let uuid = match row.get::<_, Uuid>(0) {
                Ok(uuid) => uuid,
                Err(err) => {
                    ::log::error!("could not get uuid of\nrow: {row:#?}\n{err}");
                    continue;
                }
            };

            let Some(config) = get_column_blob(row, 1, "config", &uuid) else {
                continue;
            };

            buf.clear();
            let mut config_reader = GzDecoder::new(config);

            if let Err(err) = config_reader.read_to_end(&mut buf) {
                ::log::error!("could not decompress config for {uuid}\n{err}");
                continue;
            }

            let config = match ::toml::from_slice::<NativeGame>(&buf) {
                Ok(config) => config,
                Err(err) => {
                    ::log::error!("could not parse game config of {uuid}\n{err}");
                    continue;
                }
            };

            let image = get_column_blob(row, 2, "image", &uuid).and_then(|image| {
                ::image::load_from_memory_with_format(image, ImageFormat::Png)
                    .map_err(|err| ::log::error!("could not load thumbnail for {uuid}\n{err}"))
                    .ok()
            });

            games.push((uuid, config, image));
        }

        Ok(games)
    }

    /// Convert an r2d2 pool reference.
    pub fn from_ref(pool: &::r2d2::Pool<SqliteConnectionManager>) -> &Self {
        TransparentWrapper::wrap_ref(pool)
    }
}
