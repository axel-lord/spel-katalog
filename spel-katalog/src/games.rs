use ::std::{
    convert::identity,
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
use ::itertools::Itertools;
use ::rustc_hash::FxHashMap;
use ::spel_katalog_common::{OrRequest, StatusSender, async_status, status, w};
use ::spel_katalog_games::{Game, Games};
use ::spel_katalog_settings::Settings;
use ::tap::{Pipe, Tap};

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
}

#[derive(Debug, ::thiserror::Error)]
pub enum LoadDbError {
    #[error("an sqlite error occurred\n{0}")]
    Sqlite(#[from] ::sqlite::Error),
}

fn load_db(path: &Path) -> Result<Vec<Game>, LoadDbError> {
    let db = ::sqlite::open(path)?;

    let _cats = db
        .prepare("SELECT id, name FROM categories")?
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

            Game::from_row(&row)
        })
        .collect::<Vec<_>>()
        .tap_mut(|games| games.sort_by_key(|game| -game.id))
        .pipe(Ok)
}

impl State {
    pub fn update(
        &mut self,
        msg: Message,
        tx: &StatusSender,
        settings: &Settings,
        filter: &str,
    ) -> Task<crate::Message> {
        match msg {
            Message::LoadDb(path_buf) => {
                let tx = tx.clone();
                Task::future(async move {
                    match ::tokio::task::spawn_blocking(move || load_db(&path_buf)).await {
                        Ok(result) => match result {
                            Ok(games) => games
                                .pipe(Message::SetGames)
                                .pipe(OrRequest::Message)
                                .pipe(crate::Message::Games)
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

                let slugs = self.all().iter().map(|game| &game.slug).cloned().collect();
                status!(tx, "read games from database");

                Task::done(crate::Message::FindImages { slugs })
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

    pub fn view(&self) -> Element<OrRequest<Message, Request>> {
        fn card<'a>(game: &'a Game, width: f32) -> Element<'a, OrRequest<Message, Request>> {
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
            .on_release(OrRequest::Request(Request::SetId { id: game.id }))
            .on_middle_release(OrRequest::Request(Request::Run {
                id: game.id,
                sandbox: true,
            }))
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
        .pipe(Element::from)
    }
}
