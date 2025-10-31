use ::std::{
    borrow::Cow,
    collections::hash_map::Entry,
    fs::ReadDir,
    path::{Path, PathBuf},
};

use ::futures::Stream;
use ::image::{DynamicImage, imageops::FilterType::Lanczos3};
use ::rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use ::rustc_hash::FxHashMap;

/// Options used for gathering covers.
#[derive(Debug, Clone)]
pub struct CoverGathererOptions {
    /// Iterable of slugs to find covers for.
    /// If none find all covers in directory.
    /// (default: None)
    pub slugs: Option<Vec<String>>,

    /// Size of bounded channel found covers are sent through.
    /// If 0 use an unbounded channel.
    /// (default: 64)
    pub channel_size: usize,

    /// Max Width and height to scale cover to.
    /// If 0 do not scale.
    /// (default: 150)
    pub dimensions: u32,
}

impl Default for CoverGathererOptions {
    fn default() -> Self {
        Self {
            slugs: None,
            channel_size: 64,
            dimensions: 150,
        }
    }
}

/// Iterator or Stream receiving gathered covers.
#[derive(Debug, Clone)]
pub struct CoverGatherer {
    receiver: ::flume::Receiver<(String, ::spel_katalog_formats::Image)>,
}

fn gather_covers(dir: ReadDir) -> FxHashMap<String, PathBuf> {
    dir.par_bridge()
        .filter_map(|dir_entry| {
            let entry = dir_entry.ok()?;
            let path = entry.path();

            let slug = path
                .with_extension("")
                .into_os_string()
                .into_string()
                .ok()?;

            let metadata = entry
                .metadata()
                .map_err(|err| ::log::warn!("could not get metadata for {path:?}\n{err}"))
                .ok()?;

            metadata.is_file().then_some((slug, path))
        })
        .collect::<Vec<_>>()
        .into_iter()
        .fold(FxHashMap::default(), |mut m, (k, v)| {
            match m.entry(k) {
                Entry::Vacant(entry) => {
                    entry.insert(v);
                }
                // If duplicate either do nothing or insert if new current is png.
                Entry::Occupied(mut entry)
                    if v.extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("png")) =>
                {
                    ::log::warn!(
                        "duplicate covers available for {}, using {v:?}",
                        entry.key()
                    );
                    entry.insert(v);
                }
                Entry::Occupied(entry) => {
                    ::log::warn!(
                        "duplicate covers available for {}, using {:?}",
                        entry.key(),
                        entry.get()
                    );
                }
            }
            m
        })
}

/// Process a single image into a thumbnail.
pub fn thumbnail(image: DynamicImage, dimensions: u32) -> ::spel_katalog_formats::Image {
    let image = if dimensions != 0 && (image.width() > dimensions || image.height() > dimensions) {
        image.resize(dimensions, dimensions, Lanczos3).into_rgba8()
    } else {
        image.into_rgba8()
    };

    ::spel_katalog_formats::Image {
        width: image.width(),
        height: image.height(),
        bytes: image.into_raw().into(),
    }
}

impl CoverGatherer {
    /// Create a new CoverGatherer for finding the given slugs.
    pub fn new(cover_dir: &Path, slugs: Vec<String>) -> ::std::io::Result<Self> {
        Self::with_options(
            cover_dir,
            CoverGathererOptions {
                slugs: Some(slugs),
                ..Default::default()
            },
        )
    }

    /// Create a new CoverGatherer for the given path and covers using the provided options.
    pub fn with_options(
        cover_dir: &Path,
        options: CoverGathererOptions,
    ) -> ::std::io::Result<Self> {
        let CoverGathererOptions {
            slugs,
            channel_size,
            dimensions,
        } = options;
        let dir = ::std::fs::read_dir(cover_dir)?;
        let (tx, rx) = if options.channel_size == 0 {
            ::flume::unbounded()
        } else {
            ::flume::bounded(channel_size)
        };

        ::rayon::spawn(move || {
            let covers = gather_covers(dir);
            let covers = if let Some(slugs) = slugs {
                slugs
                    .into_iter()
                    .filter_map(|slug| {
                        let path = covers.get(&slug)?.as_path();
                        Some((slug, Cow::Borrowed(path)))
                    })
                    .collect::<Vec<_>>()
            } else {
                covers
                    .into_iter()
                    .map(|(key, value)| (key, Cow::Owned(value)))
                    .collect::<Vec<_>>()
            };

            let result = covers
                .into_par_iter()
                .try_fold(
                    || 0usize,
                    |c, (slug, path)| {
                        let path = path.as_ref();
                        let image = match ::image::open(path) {
                            Err(err) => {
                                ::log::warn!(
                                    "could not read image for {slug} from {path:?}\n{err}"
                                );
                                return Ok(c);
                            }
                            Ok(image) => image,
                        };

                        let image = thumbnail(image, dimensions);

                        match tx.send((slug, image)) {
                            Ok(..) => Ok(c + 1),
                            Err(err) => {
                                let slug = &err.0.0;
                                ::log::warn!("could not send image for {slug}\n{err}");
                                Err(())
                            }
                        }
                    },
                )
                .try_reduce(|| 0, |a, b| Ok(a + b));
            if let Ok(result) = result {
                ::log::info!("loaded {result} covers from filesystem");
            }
        });

        Ok(Self { receiver: rx })
    }

    /// Convert into a stream of slugs and images.
    pub fn into_stream(self) -> impl Stream<Item = (String, ::spel_katalog_formats::Image)> {
        let Self { receiver } = self;
        receiver.into_stream()
    }
}

impl IntoIterator for CoverGatherer {
    type Item = (String, ::spel_katalog_formats::Image);

    type IntoIter = ::flume::IntoIter<(String, ::spel_katalog_formats::Image)>;

    fn into_iter(self) -> Self::IntoIter {
        self.receiver.into_iter()
    }
}

impl IntoIterator for &CoverGatherer {
    type Item = (String, ::spel_katalog_formats::Image);

    type IntoIter = ::flume::IntoIter<(String, ::spel_katalog_formats::Image)>;

    fn into_iter(self) -> Self::IntoIter {
        self.receiver.clone().into_iter()
    }
}

impl IntoIterator for &mut CoverGatherer {
    type Item = (String, ::spel_katalog_formats::Image);

    type IntoIter = ::flume::IntoIter<(String, ::spel_katalog_formats::Image)>;

    fn into_iter(self) -> Self::IntoIter {
        self.receiver.clone().into_iter()
    }
}
