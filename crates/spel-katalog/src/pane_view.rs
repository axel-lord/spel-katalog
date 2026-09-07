//! Implementation of pane view window.

use ::iced_core::window;
use ::iced_runtime::Task;
use ::iced_widget::{container, pane_grid, text};
use ::tap::Pipe;

/// View panes.
#[derive(Debug, Clone, Copy)]
pub struct PaneView<'a> {
    /// Reference to pane view state.
    pub state: &'a State,
    /// Reference to settings view state.
    pub settings: &'a ::spel_katalog_settings_view::State,
    /// Reference to terminal state.
    pub terminal: &'a ::spel_katalog_terminal::Terminal,
    /// Reference to process view state.
    pub process_view: &'a ::spel_katalog_process_view::ProcessView,
    /// Reference to game info state.
    pub info: &'a ::spel_katalog_info::State,
    /// Reference to games view state.
    pub games: &'a ::spel_katalog_games::State,
}

impl<'a> PaneView<'a> {
    /// View panes.
    pub fn view(
        self,
        id: window::Id,
    ) -> ::iced_core::Element<'a, crate::Message, ::iced_core::Theme, ::iced_widget::Renderer> {
        ::iced_widget::pane_grid(&self.state.panes, |_pane, state, _is_maximized| {
            pane_grid::Content::new(match state {
                Pane::Log => self
                    .terminal
                    .log_view()
                    .view()
                    .map(::spel_katalog_terminal::Message::LogView)
                    .map(crate::Message::Terminal),
                Pane::Terminal => self.terminal.view().map(crate::Message::Terminal),
                Pane::Settings => self.settings.view().map(crate::Message::Settings),
                Pane::Processes => self.process_view.view().map(crate::Message::ProcessView),
                Pane::GameInfo => {
                    if let Some(id) = self.info.id()
                        && let Some(game) = self.games.by_id(id)
                    {
                        ::iced_widget::Column::new()
                            .spacing(3)
                            .padding(3)
                            .push(self.info.titlebar(
                                game,
                                game.thumb.as_ref(),
                                id,
                                game.shadows,
                                ::iced_widget::space().into(),
                            ))
                            .into()
                    } else {
                        container(text("No game is currently selected!"))
                            .style(container::rounded_box)
                            .into()
                    }
                }
            })
        })
        .on_resize(5, move |event| {
            crate::Message::PaneView(id, Message::Resize(event))
        })
        .on_drag(move |event| crate::Message::PaneView(id, Message::Drag(event)))
        .spacing(3)
        .pipe(::iced_widget::container)
        .padding(3)
        .into()
    }
}

/// A pane view window/widget.
#[derive(Debug, Clone)]
pub struct State {
    /// Pane grid state.
    panes: pane_grid::State<Pane>,
}

impl State {
    /// Create a state builder.
    pub fn builder(initial: Pane) -> Builder {
        let (state, offset) = pane_grid::State::new(initial);
        Builder {
            offset,
            panes: state,
        }
    }

    /// Update state.
    pub fn update(&mut self, message: Message) -> Task<crate::Message> {
        match message {
            Message::Resize(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Task::none()
            }
            Message::Drag(drag_event) => match drag_event {
                pane_grid::DragEvent::Dropped { pane, target } => {
                    self.panes.drop(pane, target);
                    Task::none()
                }
                pane_grid::DragEvent::Canceled { pane: _ }
                | pane_grid::DragEvent::Picked { pane: _ } => Task::none(),
            },
        }
    }

    /// Create new state.
    pub fn new(initial: Pane) -> Self {
        Self::builder(initial).build()
    }
}

/// State builder for pane view.
#[derive(Debug, Clone)]
pub struct Builder {
    /// Last added pane.
    offset: pane_grid::Pane,
    /// Current state.
    panes: pane_grid::State<Pane>,
}

impl Builder {
    /// Build state.
    pub fn build(self) -> State {
        let Self { offset: _, panes } = self;
        State { panes }
    }

    /// Perform a split across the given axis.
    pub fn split(self, pane: Pane, axis: pane_grid::Axis) -> Self {
        let Self { offset, mut panes } = self;
        let offset = panes
            .split(axis, offset, pane)
            .map_or(offset, |(offset, _)| offset);
        Self { offset, panes }
    }

    /// Perform a horizontal split and add given pane.
    pub fn horizontal(self, pane: Pane) -> Self {
        self.split(pane, pane_grid::Axis::Horizontal)
    }

    /// Perform a vertical split and add given pane.
    pub fn vertical(self, pane: Pane) -> Self {
        self.split(pane, pane_grid::Axis::Vertical)
    }
}

/// Kind of displayed pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Pane {
    /// Display log view.
    Log,
    /// Display terminal.
    Terminal,
    /// Display settings.
    Settings,
    /// Display processes.
    Processes,
    /// Display game info.
    GameInfo,
}

/// Message produced and consumed by pane view.
#[derive(Debug, Clone)]
pub enum Message {
    /// Pane was resized.
    Resize(pane_grid::ResizeEvent),
    /// Pane was dragged.
    Drag(pane_grid::DragEvent),
}
