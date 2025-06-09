use ::std::path::PathBuf;

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
use ::tap::{Pipe, Tap};

use crate::{Safety, settings::Settings, w};

pub use crate::games::games::{Game, Games};

mod games;

#[derive(Debug, Default, Deref, DerefMut)]
pub struct State {
    #[deref]
    #[deref_mut]
    games: Games,
}

#[derive(Debug, Clone, IsVariant)]
pub enum Message {
    LoadDb(PathBuf),
    SetGames(Vec<Game>),
    SetImages {
        slugs: Vec<String>,
        images: Vec<Handle>,
    },
    SetImage {
        slug: String,
        image: Handle,
    },
}

#[derive(Debug, ::thiserror::Error)]
pub enum LoadDbError {
    #[error("an sqlite error occurred\n{0}")]
    Sqlite(#[from] ::sqlite::Error),
}

impl State {
    pub fn update(
        &mut self,
        msg: Message,
        settings: &Settings,
        filter: &str,
    ) -> Task<crate::Message> {
        match msg {
            Message::LoadDb(path_buf) => Task::future(::tokio::task::spawn_blocking(
                move || -> Result<Vec<Game>, LoadDbError> {
                    let db = ::sqlite::open(&path_buf)?;

                    let _cats = db
                        .prepare("SELECT id, name FROM categories WHERE")?
                        .into_iter()
                        .map(|cat| {
                            let cat = cat?;
                            let id = cat.try_read::<i64, _>("id")?;
                            let name = cat.try_read::<&str, _>("name")?;

                            Ok((id, String::from(name)))
                        })
                        .collect::<Result<FxHashMap<_, _>, ::sqlite::Error>>();

                    let _game_cats = db
                        .prepare("SELECT game_id, category_id FROM games_categories")?
                        .into_iter()
                        .map(|row| {
                            let row = row?;
                            let game: i64 = row.try_read("game_id")?;
                            let cat: i64 = row.try_read("category_id")?;

                            Ok((game, cat))
                        })
                        .collect::<Result<Vec<_>, ::sqlite::Error>>();

                    db.prepare("SELECT id,name,slug,runner,configpath FROM games")?
                        .into_iter()
                        .filter_map(|row| {
                            let row = row
                                .map_err(|err| ::log::error!("row does not exist\n{err}"))
                                .ok()?;

                            let slug = row
                                .try_read::<&str, _>("slug")
                                .map_err(|err| ::log::error!("could not read slug of row\n{err}"))
                                .ok()?
                                .into();
                            let id = row
                                .try_read::<i64, _>("id")
                                .map_err(|err| ::log::error!("could not read id of row\n{err}"))
                                .ok()?
                                .into();
                            let name = row
                                .try_read::<&str, _>("name")
                                .map_err(|err| ::log::error!("could not read name of row\n{err}"))
                                .ok()?
                                .into();
                            let runner = row
                                .try_read::<&str, _>("runner")
                                .map_err(|err| ::log::error!("could not read runner of row\n{err}"))
                                .ok()?
                                .into();
                            let configpath = row
                                .try_read::<&str, _>("configpath")
                                .map_err(|err| {
                                    ::log::error!("could not read configpath of row\n{err}")
                                })
                                .ok()?
                                .into();

                            Some(Game {
                                slug,
                                id,
                                name,
                                runner,
                                configpath,
                                image: None,
                            })
                        })
                        .collect::<Vec<_>>()
                        .tap_mut(|games| games.sort_by_key(|game| -game.id))
                        .pipe(Ok)
                },
            ))
            .then(|result| match result {
                Ok(result) => match result {
                    Ok(games) => games
                        .pipe(Message::SetGames)
                        .pipe(crate::Message::Games)
                        .pipe(Task::done),
                    Err(err) => match err {
                        LoadDbError::Sqlite(_error) => {
                            Task::done(String::from("an sqlite error occurred").into())
                        }
                    },
                },
                Err(err) => {
                    ::log::error!("database thread did not finish\n{err}");
                    Task::done(String::from("thread did not finish").into())
                }
            }),
            Message::SetGames(games) => {
                self.set(games.into(), settings, filter);

                let slugs = self.all().map(|game| game.slug.clone()).collect();

                Task::batch(
                    [
                        "read games from database".to_owned().into(),
                        crate::Message::FindImages { slugs },
                    ]
                    .map(Task::done),
                )
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
        }
    }

    pub fn view(&self) -> Element<crate::Message> {
        fn card<'a>(game: &'a Game, width: f32) -> Element<'a, crate::Message> {
            let handle = game.image.as_ref();
            let name = game.name.as_str();

            let text = container(name)
                .padding(3)
                .style(container::bordered_box)
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
            .interaction(::iced::mouse::Interaction::Pointer)
            .on_release(crate::info::Message::SetId(game.id).pipe(crate::Message::from))
            .on_middle_release(crate::Message::RunGame(game.id, Safety::Firejail))
            .into()
        }

        widget::responsive(|size| {
            let columns = ((size.width as usize - 1) / 153).clamp(1, 24);
            let width = ((size.width / columns as f32) - 3.0).clamp(150.0, 300.0);

            w::scroll(
                w::col().align_x(Alignment::Start).width(Fill).extend(
                    self.displayed()
                        .chunks(columns)
                        .into_iter()
                        .map(|chunk| w::row().extend(chunk.map(|game| card(game, width))).into()),
                ),
            )
            .into()
        })
        .into()
    }
}
