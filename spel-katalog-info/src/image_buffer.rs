#![allow(missing_docs)]

use ::std::{
    mem,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use ::iced::{Task, widget::image::Handle};
use ::image::{RgbaImage, imageops::FilterType::Gaussian};
use ::rayon::iter::{IntoParallelIterator, ParallelIterator};
use ::tap::{Conv, Pipe};
use ::tokio_stream::wrappers::IntervalStream;

#[derive(Debug, Clone)]
pub struct ImageBuffer {
    values: Arc<::parking_lot::Mutex<Option<ImageBufferMsg>>>,
}

#[derive(Debug, Clone)]
pub struct ImageBufferMsg {
    slugs: Vec<String>,
    images: Vec<Handle>,
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

    pub fn take(&self) -> Option<(Vec<String>, Vec<Handle>)> {
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

    pub fn push(&self, slug: String, image: Handle) -> Result<(), ()> {
        match &mut *self.values.lock() {
            Some(ImageBufferMsg { slugs, images, .. }) => {
                slugs.push(slug);
                images.push(image);
                Ok(())
            }
            None => Err(()),
        }
    }

    pub fn process_single(path: &Path) -> Result<Handle, ::image::ImageError> {
        let resized = ::image::open(path)?
            .resize(150, 150, Gaussian)
            .conv::<RgbaImage>();
        Ok(Handle::from_rgba(
            resized.width(),
            resized.height(),
            resized.into_raw(),
        ))
    }

    pub fn process_bytes(bytes: &[u8]) -> Result<Handle, ::image::ImageError> {
        let resized = ::image::load_from_memory(bytes)?
            .resize(150, 150, Gaussian)
            .conv::<RgbaImage>();
        Ok(Handle::from_rgba(
            resized.width(),
            resized.height(),
            resized.into_raw(),
        ))
    }

    pub fn find_images(
        &mut self,
        slugs: Vec<String>,
        coverart: PathBuf,
    ) -> Task<::spel_katalog_games::Message> {
        #[derive(Debug, Default)]
        struct ImageOpenError {
            list: Vec<(PathBuf, ::image::ImageError)>,
        }

        impl ::std::error::Error for ImageOpenError {}

        impl ::std::fmt::Display for ImageOpenError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                writeln!(f, "could not process image")?;
                for (path, err) in &self.list {
                    writeln!(f, "{path:?}\n{err}")?;
                }
                Ok(())
            }
        }

        fn get_image(slug: &str, coverart: &Path) -> Result<Handle, ImageOpenError> {
            let mut open_error = ImageOpenError::default();
            for ext in ["png", "jpg", "jpeg"] {
                let path = coverart.join(slug).with_extension(ext);
                match ImageBuffer::process_single(&path) {
                    Ok(handle) => return Ok(handle),
                    Err(err) => open_error.list.push((path, err)),
                }
            }
            Err(open_error.into())
        }

        let image_buffer = ImageBuffer::default();
        let rx = ImageBuffer::clone(&image_buffer);
        let tx = ImageBuffer::clone(&image_buffer);

        self.close();
        *self = image_buffer;

        let (r_tx, r_rx) = ::tokio::sync::mpsc::channel(24);
        ::rayon::spawn(move || {
            let tx = r_tx;
            let coverart = coverart;
            _ = slugs.into_par_iter().try_for_each(|slug| {
                let image = get_image(&slug, &coverart);
                tx.blocking_send((slug, image))
            });
        });

        async fn work(
            mut rx: ::tokio::sync::mpsc::Receiver<(String, Result<Handle, ImageOpenError>)>,
            tx: ImageBuffer,
        ) {
            while let Some((slug, value)) = rx.recv().await {
                match value {
                    Ok(image) => {
                        if tx.push(slug, image).is_err() {
                            ::log::info!("image processing iterrupted");
                            return;
                        }
                    }
                    Err(err) => {
                        ::log::warn!("while processing image for {slug}\n{err}")
                    }
                }
            }
            tx.set_final();
        }

        let work = work(r_rx, tx).pipe(Task::future).then(|_| Task::none());

        let (stream, handle) = Duration::from_millis(200)
            .pipe(::tokio::time::interval)
            .pipe(IntervalStream::new)
            .pipe(Task::stream)
            .abortable();

        let stream = stream.then(move |_| {
            rx.take().map_or_else(
                || {
                    handle.abort();
                    Task::none()
                },
                |(slugs, images)| {
                    ::spel_katalog_games::Message::SetImages { slugs, images }.pipe(Task::done)
                },
            )
        });

        return Task::batch([stream, work]);
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
