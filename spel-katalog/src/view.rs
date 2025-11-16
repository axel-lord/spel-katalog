use ::std::cell::Cell;

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
    aspect_ratio: Cell<f32>,
}

#[derive(Debug, Clone, Copy, From)]
pub enum Message {
    #[from]
    Resized(pane_grid::ResizeEvent),
    Close,
}

impl State {
    pub fn info_shown(&self) -> bool {
        self.info.is_some()
    }

    pub fn new() -> Self {
        let (panes, games) = pane_grid::State::new(Pane::Games);

        let info = None;
        let aspect_ratio = Cell::new(16.0 / 9.0);

        Self {
            panes,
            games,
            info,
            aspect_ratio,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio.clamp(0.25, 0.75));
            }
            Message::Close => {
                self.show_info(false);
            }
        };
        Task::none()
    }

    pub fn show_info(&mut self, show_info: bool) {
        match self.info.take() {
            Some(info_pane) if show_info => {
                self.info = Some(info_pane);
            }
            Some(info_pane) => {
                self.panes.close(info_pane);
                self.info = None;
            }
            None if show_info => {
                if self.aspect_ratio.get() > 1.0
                    && let Some((pane, split)) =
                        self.panes
                            .split(pane_grid::Axis::Vertical, self.games, Pane::GameInfo)
                {
                    self.panes.resize(split, 0.3);
                    self.panes.swap(pane, self.games);
                    self.info = Some(pane);
                } else if let Some((pane, _split)) =
                    self.panes
                        .split(pane_grid::Axis::Horizontal, self.games, Pane::GameInfo)
                {
                    self.info = Some(pane);
                }
            }
            None => {}
        }
    }

    /// Create an element for titlebar buttons.
    fn buttons<'a>() -> Element<'a, Message> {
        widget::Row::new()
            .push(
                widget::button("Close")
                    .padding(3)
                    .style(widget::button::danger)
                    .on_press_with(|| Message::Close),
            )
            .into()
    }

    pub fn view<'app>(
        &'app self,
        games: &'app ::spel_katalog_games::State,
        info: &'app spel_katalog_info::State,
        shadowed: bool,
    ) -> Element<'app, crate::Message> {
        let style = |t: &_| styling::box_border(t).background(Color::WHITE.scale_alpha(0.025));
        widget::responsive(move |size| {
            self.aspect_ratio.set(size.width / size.height);

            pane_grid(&self.panes, |_pane, state, _is_maximized| {
                pane_grid::Content::new(
                    match state {
                        Pane::Games => games.view(shadowed).map(crate::Message::from),
                        Pane::GameInfo => {
                            if let Some((id, game)) =
                                info.id().and_then(|id| Some((id, games.by_id(id)?)))
                            {
                                widget::Column::new()
                                    .push(
                                        info.titlebar(
                                            game,
                                            game.thumb.as_ref(),
                                            id,
                                            Self::buttons(),
                                        )
                                        .map(crate::Message::from),
                                    )
                                    .push(widget::horizontal_rule(2))
                                    .push(info.view(game.thumb.is_some()).map(crate::Message::from))
                                    .spacing(3)
                                    .padding(5)
                                    .pipe(widget::container)
                                    .style(style)
                                    .height(Fill)
                                    .into()
                            } else {
                                widget::container("No Game Selected")
                                    .style(style)
                                    .center(Fill)
                                    .into()
                            }
                        }
                    }
                    .pipe(widget::container),
                )
            })
            .spacing(9)
            .on_resize(3, |event| crate::Message::View(Message::Resized(event)))
            .into()
        })
        .into()
    }
}
