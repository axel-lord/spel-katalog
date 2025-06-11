use ::derive_more::{Display, From, IsVariant};
use ::iced::{
    Element, Task,
    widget::{self, pane_grid},
};
use ::tap::Pipe;

#[derive(Debug, Default, Clone, Copy, Display, PartialEq, Eq, IsVariant, Hash)]
pub enum Pane {
    #[default]
    Games,
    Settings,
    GameInfo,
}

#[derive(Debug)]
pub struct State {
    panes: pane_grid::State<Pane>,
    games: pane_grid::Pane,
    settings: Option<pane_grid::Pane>,
    info: Option<pane_grid::Pane>,
}

#[derive(Debug, Clone, From)]
pub enum Message {
    #[from]
    Resized(pane_grid::ResizeEvent),
    Settings(bool),
    Info(bool),
}

impl State {
    pub fn show_settings(&self) -> bool {
        self.settings.is_some()
    }

    pub fn new(show_settings: bool) -> Self {
        let (mut panes, games) = pane_grid::State::new(Pane::Games);

        let ratio = 0.7;
        let settings = show_settings
            .then(|| {
                if let Some((pane, split)) =
                    panes.split(pane_grid::Axis::Vertical, games, Pane::Settings)
                {
                    panes.resize(split, ratio);
                    Some(pane)
                } else {
                    None
                }
            })
            .flatten();
        let info = None;

        Self {
            panes,
            games,
            settings,
            info,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            }
            Message::Settings(show_settings) => {
                if let Some(settings_pane) = self.settings.take() {
                    if show_settings {
                        self.settings = show_settings.then_some(settings_pane);
                    } else {
                        self.panes.close(settings_pane);
                    }
                } else if show_settings {
                    if let Some((pane, split)) =
                        self.panes
                            .split(pane_grid::Axis::Vertical, self.games, Pane::Settings)
                    {
                        self.panes.resize(split, 0.7);
                        self.settings = Some(pane);
                    }
                }
            }
            Message::Info(show_info) => match self.info.take() {
                Some(info_pane) => {
                    if show_info {
                        self.info = show_info.then_some(info_pane);
                    } else {
                        self.panes.close(info_pane);
                    }
                }
                None => {
                    if let Some((pane, _split)) =
                        self.panes
                            .split(pane_grid::Axis::Vertical, self.games, Pane::GameInfo)
                    {
                        self.panes.swap(pane, self.games);
                        self.info = Some(pane);
                    }
                }
            },
        };
        Task::none()
    }

    pub fn view<'app>(
        &'app self,
        settings: &'app ::spel_katalog_settings::State,
        games: &'app ::spel_katalog_games::State,
        info: &'app crate::info::State,
    ) -> Element<'app, crate::Message> {
        pane_grid(&self.panes, |_pane, state, _is_maximized| {
            pane_grid::Content::new(
                match state {
                    Pane::Games => games.view().map(crate::Message::from),
                    Pane::Settings => settings.view().map(crate::Message::from),
                    Pane::GameInfo => info.view(settings, games).map(crate::Message::from),
                }
                .pipe(widget::container),
            )
        })
        .spacing(9)
        .on_resize(3, |event| crate::Message::View(Message::Resized(event)))
        .into()
    }
}
