//! Implementation of pane view window.

use ::iced_core::{Function, window};
use ::iced_runtime::Task;
use ::iced_widget::{Space, container, pane_grid, text};
use ::spel_katalog_fold_with::Fold;
use ::spel_katalog_functional::LazyPipe;
use ::spel_katalog_list_menu::ListMenu;
use ::spel_katalog_widget::WidgetExt;
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
    /// Reference to log state.
    pub log_view: &'a ::spel_katalog_log_view::LogView,
}

fn context_menu<'a>(
    id: window::Id,
    pane: pane_grid::Pane,
    state: Pane,
    is_maximized: bool,
) -> ListMenu<'a, crate::Message> {
    ListMenu::new()
        .label("Pane")
        .separator()
        .button(
            if is_maximized { "Restore" } else { "Maximize" },
            move || {
                crate::Message::PaneView(
                    id,
                    if is_maximized {
                        Message::Restore
                    } else {
                        Message::Maximize(pane)
                    },
                )
            },
        )
        .button(
            "Close",
            pane.then_pipe(Message::Close)
                .then_chain(crate::Message::PaneView.with(id)),
        )
        .separator()
        .label("Split")
        .separator()
        .button(
            "Horizontal",
            pane.then_pipe(Message::Split.with(pane_grid::Axis::Horizontal))
                .then_chain(crate::Message::PaneView.with(id)),
        )
        .button(
            "Vertical",
            pane.then_pipe(Message::Split.with(pane_grid::Axis::Vertical))
                .then_chain(crate::Message::PaneView.with(id)),
        )
        .separator()
        .label("View")
        .separator()
        .fold_with(
            [Pane::Log, Pane::Terminal, Pane::Processes, Pane::Settings],
            move |list, pane_kind| {
                list.button_if(
                    pane_kind != state,
                    pane_kind.name(),
                    Message::SetState {
                        pane,
                        state: pane_kind,
                    }
                    .then_pipe(crate::Message::PaneView.with(id)),
                )
            },
        )
}

impl<'a> PaneView<'a> {
    /// View panes.
    pub fn view(
        self,
        id: window::Id,
    ) -> ::iced_core::Element<'a, crate::Message, ::iced_core::Theme, ::iced_widget::Renderer> {
        ::iced_widget::pane_grid(&self.state.panes, |pane, state, is_maximized| {
            pane_grid::Content::new(match state {
                Pane::Empty => ::iced_aw::ContextMenu::new(
                    Space::new()
                        .pipe(::iced_widget::center)
                        .style(container::bordered_box),
                    move || context_menu(id, pane, *state, is_maximized).into(),
                )
                .pipe(crate::Element::from),
                Pane::Log => self
                    .log_view
                    .view()
                    .map(crate::Message::LogView)
                    .with_wrapper(::iced_aw::ContextMenu::new, move || {
                        context_menu(id, pane, *state, is_maximized)
                            .join_separated(
                                self.log_view.context_menu().map(crate::Message::LogView),
                            )
                            .into()
                    })
                    .into(),
                Pane::Terminal => self
                    .terminal
                    .view()
                    .map(crate::Message::Terminal)
                    .with_wrapper(::iced_aw::ContextMenu::new, move || {
                        context_menu(id, pane, *state, is_maximized).into()
                    })
                    .into(),
                Pane::Settings => self
                    .settings
                    .view()
                    .map(crate::Message::Settings)
                    .with_wrapper(::iced_aw::ContextMenu::new, move || {
                        context_menu(id, pane, *state, is_maximized).into()
                    })
                    .into(),
                Pane::Processes => self
                    .process_view
                    .view()
                    .map(crate::Message::ProcessView)
                    .with_wrapper(::iced_aw::ContextMenu::new, move || {
                        context_menu(id, pane, *state, is_maximized)
                            .join_separated(
                                self.process_view
                                    .context_menu()
                                    .map(crate::Message::ProcessView),
                            )
                            .into()
                    })
                    .into(),
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
            Message::Close(pane) => {
                if self.panes.len() <= 1 {
                    (self.panes, _) = pane_grid::State::new(Pane::Empty);
                } else {
                    self.panes.close(pane);
                }

                Task::none()
            }
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
            Message::Maximize(pane) => {
                self.panes.maximize(pane);
                Task::none()
            }
            Message::Restore => {
                self.panes.restore();
                Task::none()
            }
            Message::SetState { pane, state } => {
                if let Some(pane) = self.panes.get_mut(pane) {
                    *pane = state;
                }
                Task::none()
            }
            Message::Split(axis, pane) => {
                self.panes.split(axis, pane, Pane::Empty);
                Task::none()
            }
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
    /// Displau empty pane.
    Empty,
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

impl Pane {
    /// Get name of a pane.
    pub const fn name(self) -> &'static str {
        match self {
            Pane::Log => "Log View",
            Pane::Terminal => "Terminal",
            Pane::Settings => "Settings",
            Pane::Processes => "Processes",
            Pane::GameInfo => "Game Info",
            Pane::Empty => "Empty",
        }
    }
}

/// Message produced and consumed by pane view.
#[derive(Debug, Clone)]
pub enum Message {
    /// Close a pane.
    Close(pane_grid::Pane),
    /// Pane was resized.
    Resize(pane_grid::ResizeEvent),
    /// Pane was dragged.
    Drag(pane_grid::DragEvent),
    /// Pane should be maximized.
    Maximize(pane_grid::Pane),
    /// Split a pane.
    Split(pane_grid::Axis, pane_grid::Pane),
    /// Maximized pane should be minimized.
    Restore,
    /// Set state of pane.
    SetState {
        /// Pane to set state of.
        pane: pane_grid::Pane,
        /// State to set pane to.
        state: Pane,
    },
}
