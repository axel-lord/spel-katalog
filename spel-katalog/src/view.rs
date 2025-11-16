use ::std::cell::Cell;

use ::derive_more::{Display, From, IsVariant};
use ::iced_core::{
    Alignment, Color,
    Length::Fill,
    alignment::{Horizontal, Vertical},
};
use ::iced_runtime::Task;
use ::iced_widget::{self as widget, pane_grid};
use ::spel_katalog_common::styling;
use ::tap::Pipe;

use crate::{Element, process_info::ProcessInfo};

#[derive(Debug, Default, Clone, Copy, Display, PartialEq, Eq, IsVariant, Hash)]
pub enum Pane {
    #[default]
    Games,
    GameInfo,
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, IsVariant, Hash, Default)]
pub enum Displayed {
    #[default]
    GameInfo,
    Processes,
}

#[derive(Debug)]
pub struct State {
    panes: pane_grid::State<Pane>,
    games: pane_grid::Pane,
    info: Option<pane_grid::Pane>,
    aspect_ratio: Cell<f32>,
    pub displayed: Displayed,
}

#[derive(Debug, Clone, Copy, From)]
pub enum Message {
    #[from]
    Resized(pane_grid::ResizeEvent),
    Close,
    SetDisplayed(Displayed),
}

impl State {
    pub fn info_shown(&self) -> bool {
        self.info.is_some()
    }

    pub fn new() -> Self {
        let (panes, games) = pane_grid::State::new(Pane::Games);

        let info = None;
        let aspect_ratio = Cell::new(16.0 / 9.0);
        let displayed = Default::default();

        Self {
            panes,
            games,
            info,
            aspect_ratio,
            displayed,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::Resized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio.clamp(0.25, 0.75));
            }
            Message::Close => {
                self.hide_info();
            }
            Message::SetDisplayed(displayed) => {
                self.displayed = displayed;
            }
        };
        Task::none()
    }

    pub fn show_info(&mut self) {
        let Self {
            panes,
            games,
            info,
            aspect_ratio,
            ..
        } = self;
        if let info @ None = info {
            if aspect_ratio.get() > 1.0
                && let Some((pane, split)) =
                    panes.split(pane_grid::Axis::Vertical, *games, Pane::GameInfo)
            {
                panes.resize(split, 0.3);
                panes.swap(pane, self.games);
                *info = Some(pane);
            } else if let Some((pane, _split)) =
                panes.split(pane_grid::Axis::Horizontal, *games, Pane::GameInfo)
            {
                *info = Some(pane);
            }
        }
    }

    pub fn hide_info(&mut self) {
        let Self { panes, info, .. } = self;
        if let Some(pane) = info {
            panes.close(*pane);
            *info = None;
        }
    }

    /// Create an element for titlebar buttons.
    fn buttons<'a>(&'a self) -> Element<'a, Message> {
        widget::Row::new()
            .spacing(3)
            .push(
                widget::pick_list(
                    [Displayed::GameInfo, Displayed::Processes],
                    Some(self.displayed),
                    Message::SetDisplayed,
                )
                .padding(3),
            )
            .push(
                widget::button("Close")
                    .padding(3)
                    .style(widget::button::danger)
                    .on_press_with(|| Message::Close),
            )
            .into()
    }

    /// Create a titlebar
    fn titlebar<'app>(&'app self) -> Element<'app, crate::Message> {
        widget::Row::new()
            .spacing(3)
            .align_y(Alignment::Center)
            .push(
                widget::text(match self.displayed {
                    Displayed::GameInfo => "No Game Selected",
                    Displayed::Processes => "Processes",
                })
                .width(Fill)
                .align_y(Vertical::Center)
                .align_x(Horizontal::Center),
            )
            .push(self.buttons().map(crate::Message::from))
            .into()
    }

    fn view_info<'app>(
        &'app self,
        games: &'app ::spel_katalog_games::State,
        info: &'app spel_katalog_info::State,
        process_info: &'app [ProcessInfo],
    ) -> Element<'app, crate::Message> {
        let style = |t: &_| styling::box_border(t).background(Color::WHITE.scale_alpha(0.025));
        match self.displayed {
            Displayed::GameInfo => {
                if let Some((id, game)) = info.id().and_then(|id| Some((id, games.by_id(id)?))) {
                    widget::Column::new()
                        .push(
                            info.titlebar(game, game.thumb.as_ref(), id, self.buttons())
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
                    widget::Column::new()
                        .push(self.titlebar())
                        .push(widget::horizontal_rule(2))
                        .push(widget::vertical_space())
                        .padding(5)
                        .spacing(3)
                        .pipe(widget::container)
                        .style(style)
                        .into()
                }
            }
            Displayed::Processes => widget::Column::new()
                .push(self.titlebar())
                .push(widget::horizontal_rule(2))
                .push(ProcessInfo::view_list(process_info))
                .padding(5)
                .spacing(3)
                .pipe(widget::container)
                .style(style)
                .into(),
        }
    }

    pub fn view<'app>(
        &'app self,
        games: &'app ::spel_katalog_games::State,
        info: &'app spel_katalog_info::State,
        process_info: &'app [ProcessInfo],
        shadowed: bool,
    ) -> Element<'app, crate::Message> {
        widget::responsive(move |size| {
            self.aspect_ratio.set(size.width / size.height);

            pane_grid(&self.panes, |_pane, state, _is_maximized| {
                pane_grid::Content::new(
                    match state {
                        Pane::Games => games.view(shadowed).map(crate::Message::from),
                        Pane::GameInfo => self.view_info(games, info, process_info),
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
