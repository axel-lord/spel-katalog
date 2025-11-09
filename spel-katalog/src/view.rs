use ::derive_more::{Display, From, IsVariant};
use ::iced_core::{Color, Length::Fill};
use ::iced_runtime::Task;
use ::iced_widget::{self as widget, pane_grid};
use ::spel_katalog_common::styling;
use ::tap::Pipe;

use crate::Element;

#[derive(Debug, Default, Clone, Copy, Display, PartialEq, Eq, IsVariant, Hash)]
pub enum Pane {
    #[default]
    Games,
    GameInfo,
}

#[derive(Debug)]
pub struct State {
    panes: pane_grid::State<Pane>,
    games: pane_grid::Pane,
    info: Option<pane_grid::Pane>,
}

#[derive(Debug, Clone, From)]
pub enum Message {
    #[from]
    Resized(pane_grid::ResizeEvent),
}

impl State {
    pub fn info_shown(&self) -> bool {
        self.info.is_some()
    }

    pub fn new() -> Self {
        let (panes, games) = pane_grid::State::new(Pane::Games);

        let info = None;

        Self { panes, games, info }
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
            }
        };
        Task::none()
    }

    pub fn show_info(&mut self, show_info: bool) {
        match self.info.take() {
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
        }
    }

    pub fn view<'app>(
        &'app self,
        games: &'app ::spel_katalog_games::State,
        info: &'app spel_katalog_info::State,
        shadowed: bool,
    ) -> Element<'app, crate::Message> {
        let style = |t: &iced_core::Theme| {
            styling::box_border(t).background(Color::WHITE.scale_alpha(0.025))
        };
        pane_grid(&self.panes, |_pane, state, _is_maximized| {
            pane_grid::Content::new(
                match state {
                    Pane::Games => games.view(shadowed).map(crate::Message::from),
                    Pane::GameInfo => info
                        .view(|id| games.by_id(id).map(|g| (&g.game, g.thumb.as_ref())))
                        .map(crate::Message::from)
                        .pipe(widget::container)
                        .padding(5)
                        .style(style)
                        .height(Fill)
                        .into(),
                }
                .pipe(widget::container),
            )
        })
        .spacing(9)
        .on_resize(3, |event| crate::Message::View(Message::Resized(event)))
        .into()
    }
}
