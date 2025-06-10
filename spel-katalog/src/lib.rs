use ::std::{ffi::OsString, path::PathBuf};

use ::clap::Parser;
use ::color_eyre::{Report, Section, eyre::eyre};
use ::derive_more::{From, IsVariant};
use ::iced::{
    Element,
    Length::Fill,
    Task,
    widget::{self, horizontal_rule, text, text_input, toggler, value, vertical_space},
};
use ::log::info;
use ::spel_katalog_common::{OrStatus, status, w};
use ::tap::{Pipe, TryConv};

use crate::image_buffer::ImageBuffer;

mod games;
mod image_buffer;
mod info;
mod view;

pub mod t;
pub mod y;

#[derive(Debug, Parser)]
#[command(author, version)]
pub struct Cli {
    #[command(flatten)]
    pub settings: ::spel_katalog_settings::Settings,

    /// Show settings at startup.
    #[arg(long)]
    pub show_settings: bool,

    /// Print a skeleton config.
    #[arg(long)]
    pub skeleton: bool,

    /// Config file to load.
    #[arg(long, short)]
    pub config: Option<PathBuf>,
}

impl TryFrom<Cli> for App {
    type Error = ::color_eyre::Report;
    fn try_from(value: Cli) -> Result<Self, Self::Error> {
        let Cli {
            settings,
            show_settings,
            config,
            skeleton,
        } = value;

        let overrides = settings;
        let settings = if let Some(config) = &config {
            ::std::fs::read_to_string(config)
                .map_err(|err| {
                    eyre!(err).suggestion(format!("does {config:?} exist, and is it readable"))
                })?
                .pipe_deref(::toml::from_str::<::spel_katalog_settings::Settings>)
                .map_err(|err| eyre!(err).suggestion(format!("is {config:?} a toml file")))?
                .apply(::spel_katalog_settings::Delta::create(overrides.clone()))
        } else {
            overrides.clone()
        };

        if skeleton {
            ::std::io::copy(
                &mut ::std::io::Cursor::new(
                    ::toml::to_string_pretty(&settings.skeleton()).map_err(|err| eyre!(err))?,
                ),
                &mut ::std::io::stdout().lock(),
            )
            .map_err(|err| eyre!(err))?;

            ::std::process::exit(0)
        }

        let filter = String::new();
        let status = String::new();
        let view = view::State::new(show_settings);
        let settings = ::spel_katalog_settings::State { settings, config };
        let games = games::State::default();
        let image_buffer = ImageBuffer::empty();
        let info = info::State::default();

        Ok(App {
            settings,
            status,
            filter,
            view,
            games,
            image_buffer,
            info,
        })
    }
}

#[derive(Debug)]
pub struct App {
    settings: ::spel_katalog_settings::State,
    games: games::State,
    status: String,
    filter: String,
    view: view::State,
    info: info::State,
    image_buffer: ImageBuffer,
}

#[derive(Debug, Clone, Copy, Default, IsVariant, PartialEq, Eq, Hash)]
pub enum Safety {
    None,
    #[default]
    Firejail,
}

#[derive(Debug, IsVariant, From, Clone)]
pub enum Message {
    Filter(String),
    #[from]
    Settings(::spel_katalog_settings::Message),
    #[from]
    View(view::Message),
    #[from]
    Games(games::Message),
    #[from]
    Info(info::Message),
    FindImages {
        slugs: Vec<String>,
    },
    RunGame(i64, Safety),
}

impl App {
    pub fn run() -> ::color_eyre::Result<()> {
        ::color_eyre::install()?;
        ::env_logger::builder()
            .filter_module("spel_katalog", ::log::LevelFilter::Debug)
            .init();
        let app = Cli::parse().try_conv::<Self>()?;

        fn view(app: &App) -> Element<OrStatus<Message>> {
            app.view().map(OrStatus::new)
        }

        ::iced::application("Lutris Games", Self::update, view)
            .theme(|app| ::iced::Theme::from(*app.settings.settings.theme()))
            .centered()
            .executor::<::tokio::runtime::Runtime>()
            .run_with(|| {
                let task = app
                    .settings
                    .lutris_db()
                    .as_path()
                    .to_path_buf()
                    .pipe(games::Message::LoadDb)
                    .pipe(crate::Message::from)
                    .pipe(OrStatus::new)
                    .pipe(Task::done);
                (app, task)
            })
            .map_err(Report::from)
    }

    pub fn update(&mut self, msg: OrStatus<Message>) -> Task<OrStatus<Message>> {
        let msg = match msg {
            OrStatus::Message(msg) => msg,
            OrStatus::Status(status) => {
                info!("status: {status}");
                self.status = status;
                return Task::none();
            }
        };
        match msg {
            Message::Filter(filter) => {
                self.filter = filter;
                self.games.sort(&self.settings, &self.filter);
            }
            Message::Settings(message) => {
                let re_sort = matches!(
                    &message,
                    ::spel_katalog_settings::Message::Delta(
                        ::spel_katalog_settings::Delta::FilterMode(..)
                            | ::spel_katalog_settings::Delta::Show(..)
                            | ::spel_katalog_settings::Delta::SortBy(..)
                            | ::spel_katalog_settings::Delta::SortDir(..)
                    )
                );

                let task = self.settings.update(message);

                if re_sort {
                    self.games.sort(&self.settings, &self.filter);
                }

                return task.map(OrStatus::convert);
            }
            Message::View(message) => return self.view.update(message).map(OrStatus::new),
            Message::Games(message) => {
                return self.games.update(message, &self.settings, &self.filter);
            }
            Message::FindImages { slugs } => {
                let coverart = self.settings.coverart_dir().as_path().to_path_buf();
                return self
                    .image_buffer
                    .find_images(slugs, coverart)
                    .map(OrStatus::new);
            }
            Message::Info(message) => {
                return self.info.update(message, &self.settings, &self.games);
            }
            Message::RunGame(id, safety) => {
                let Some(game) = self.games.by_id(id) else {
                    return Task::done(status!("could not run game with id {id}"));
                };

                let lutris = self.settings.lutris_exe().clone();
                let firejail = self.settings.firejail_exe().clone();
                let slug = game.slug.clone();
                let name = game.name.clone();
                let is_net_disabled = self.settings.network().is_disabled();
                let configpath = self
                    .settings
                    .yml_dir()
                    .as_path()
                    .join(&game.configpath)
                    .with_extension("yml");

                return Task::future(async move {
                    let rungame = format!("lutris:rungame/{slug}");

                    let status = match safety {
                        Safety::None => {
                            ::tokio::process::Command::new(lutris)
                                .arg(rungame)
                                .kill_on_drop(true)
                                .status()
                                .await
                        }
                        Safety::Firejail => {
                            let common = ::tokio::fs::read_to_string(&configpath)
                                .await
                                .map_err(|err| {
                                    ::log::error!("could not read {configpath:?}\n{err}")
                                })
                                .ok()
                                .and_then(|content| {
                                    ::serde_yml::from_str::<y::Config>(&content)
                                        .map_err(|err| {
                                            ::log::error!("could not parse {configpath:?}\n{err}")
                                        })
                                        .ok()
                                })
                                .map(|config| config.game.common_parent())
                                .unwrap_or_else(|| ::spel_katalog_settings::HOME.as_path().into());
                            let mut arg = OsString::from("--whitelist=");
                            arg.push(common);
                            arg.push("/");

                            ::tokio::process::Command::new(firejail)
                                .arg(arg)
                                .args(is_net_disabled.then_some("--net=none"))
                                .arg(lutris)
                                .arg(rungame)
                                .kill_on_drop(true)
                                .status()
                                .await
                        }
                    };

                    match status {
                        Ok(status) => format!("{name} exited with {status}").into(),
                        Err(err) => {
                            ::log::error!("could not run {slug}\n{err}");
                            format!("could not run {slug}").into()
                        }
                    }
                });
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        w::col()
            .padding(5)
            .spacing(0)
            .push(
                text_input(
                    match self.settings.settings.filter_mode() {
                        ::spel_katalog_settings::FilterMode::Filter => "filter...",
                        ::spel_katalog_settings::FilterMode::Search => "search...",
                        ::spel_katalog_settings::FilterMode::Regex => "regex...",
                    },
                    &self.filter,
                )
                .width(Fill)
                .padding(3)
                .on_input(Message::Filter),
            )
            .push(vertical_space().height(5))
            .push(self.view.view(&self.settings, &self.games, &self.info))
            .push(horizontal_rule(2))
            .push(
                w::row()
                    .push(text(&self.status).width(Fill))
                    .push(text("Displayed").style(widget::text::secondary))
                    .push(value(self.games.displayed_count()))
                    .push(text("All").style(widget::text::secondary))
                    .push(value(self.games.all_count()))
                    .push(text("Network").style(widget::text::secondary))
                    .push(
                        toggler(self.settings.network().is_enabled())
                            .spacing(0)
                            .on_toggle(|net| {
                                Message::Settings(::spel_katalog_settings::Message::Delta(
                                    spel_katalog_settings::Delta::Network(match net {
                                        true => spel_katalog_settings::Network::Enabled,
                                        false => spel_katalog_settings::Network::Disabled,
                                    }),
                                ))
                            }),
                    )
                    .push(text("Settings").style(widget::text::secondary))
                    .push(
                        toggler(self.view.show_settings())
                            .spacing(0)
                            .on_toggle(|show_settings| {
                                view::Message::Settings(show_settings).into()
                            }),
                    ),
            )
            .pipe(Element::from)
    }
}
