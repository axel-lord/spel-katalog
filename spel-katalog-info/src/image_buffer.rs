#![allow(missing_docs)]

use ::std::{
    mem,
    path::{Path, PathBuf},
    sync::Arc,
};

use ::iced::Task;
use ::image::{RgbaImage, imageops::FilterType::Gaussian};
use ::spel_katalog_gather::{CoverGatherer, CoverGathererOptions};
use ::spel_katalog_tracker::Tracker;
use ::tap::{Conv, Pipe};

#[derive(Debug, Clone)]
pub struct ImageBuffer {
    values: Arc<::parking_lot::Mutex<Option<ImageBufferMsg>>>,
}

#[derive(Debug, Clone)]
pub struct ImageBufferMsg {
    slugs: Vec<String>,
    images: Vec<::spel_katalog_formats::Image>,
    is_final: bool,
}

impl Default for ImageBufferMsg {
    fn default() -> Self {
        Self {
            slugs: Default::default(),
            images: Default::default(),
            is_final: false,
        }
    }
}

impl ImageBuffer {
    pub fn empty() -> Self {
        Self {
            values: Arc::new(::parking_lot::Mutex::new(None)),
        }
    }

    pub fn take(&self) -> Option<(Vec<String>, Vec<::spel_katalog_formats::Image>)> {
        match &mut *self.values.lock() {
            Some(ImageBufferMsg {
                slugs,
                images,
                is_final: false,
            }) => Some((mem::take(slugs), mem::take(images))),
            value @ Some(ImageBufferMsg { is_final: true, .. }) => value
                .take()
                .map(|ImageBufferMsg { slugs, images, .. }| (slugs, images)),
            None => None,
        }
    }

    pub fn set_final(&self) {
        match &mut *self.values.lock() {
            Some(msg) => msg.is_final = true,
            None => (),
        }
    }

    pub fn close(&self) {
        *self.values.lock() = None;
    }

    pub fn push(&self, slug: String, image: ::spel_katalog_formats::Image) -> Result<(), ()> {
        match &mut *self.values.lock() {
            Some(ImageBufferMsg { slugs, images, .. }) => {
                slugs.push(slug);
                images.push(image);
                Ok(())
            }
            None => Err(()),
        }
    }

    pub fn process_single(
        path: &Path,
    ) -> Result<::spel_katalog_formats::Image, ::image::ImageError> {
        let resized = ::image::open(path)?
            .resize(150, 150, Gaussian)
            .conv::<RgbaImage>();
        Ok(::spel_katalog_formats::Image {
            width: resized.width(),
            height: resized.height(),
            bytes: resized.into_raw().into(),
        })
    }

    pub fn process_bytes(
        bytes: &[u8],
    ) -> Result<::spel_katalog_formats::Image, ::image::ImageError> {
        let resized = ::image::load_from_memory(bytes)?
            .resize(150, 150, Gaussian)
            .conv::<RgbaImage>();
        Ok(::spel_katalog_formats::Image {
            width: resized.width(),
            height: resized.height(),
            bytes: resized.into_raw().into(),
        })
    }

    pub fn find_images(
        &mut self,
        slugs: Vec<String>,
        coverart: PathBuf,
        mut _tracker: Option<Tracker>,
    ) -> Task<::spel_katalog_games::Message> {
        CoverGatherer::with_options(
            &coverart,
            CoverGathererOptions {
                slugs: Some(slugs),
                dimensions: 150,
                ..Default::default()
            },
        )
        .map(|stream| Task::stream(stream.into_stream()))
        .ok()
        .unwrap_or_else(Task::none)
        .map(|(slug, image)| ::spel_katalog_games::Message::SetImage {
            slug,
            image,
            to_cache: true,
        })
    }
}

impl Default for ImageBuffer {
    fn default() -> Self {
        Self {
            values: ImageBufferMsg::default()
                .pipe(Some)
                .pipe(::parking_lot::Mutex::new)
                .pipe(Arc::new),
        }
    }
}
