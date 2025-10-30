use ::std::path::Path;

use ::rayon::iter::{IntoParallelIterator, ParallelIterator};
use ::rusqlite::{Connection, OpenFlags};

use crate::LoadDbError;

/// Load thumbnail database.
pub fn load_thumbnail_database(
    db_path: &Path,
) -> Result<Vec<(String, ::spel_katalog_formats::Image)>, LoadDbError> {
    let db = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let mut loaded = Vec::<(String, Vec<u8>)>::new();
    let mut stmt = db.prepare_cached("SELECT slug,image FROM images")?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let slug = match row.get("slug") {
            Ok(slug) => slug,
            Err(err) => {
                ::log::warn!(
                    "failed to read slug column as a string from a row of {db_path:?}\n{err}"
                );
                continue;
            }
        };
        let image = match row.get("image") {
            Ok(image) => image,
            Err(err) => {
                ::log::warn!(
                    "failed to read image column bytes for {slug} from  {db_path:?}\n{err}"
                );
                continue;
            }
        };

        loaded.push((slug, image));
    }

    let loaded = loaded
        .into_par_iter()
        .filter_map(|(slug, bytes)| {
            match ::image::load_from_memory_with_format(&bytes, ::image::ImageFormat::Png) {
                Ok(image) => Some((
                    slug,
                    ::spel_katalog_formats::Image {
                        width: image.width(),
                        height: image.height(),
                        bytes: image.into_rgba8().into_raw().into(),
                    },
                )),
                Err(err) => {
                    ::log::warn!("failed to read image for {slug} from {db_path:?} as png\n{err}");
                    None
                }
            }
        })
        .collect::<Vec<_>>();

    Ok(loaded)
}
