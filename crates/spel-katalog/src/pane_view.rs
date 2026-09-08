//! Implementation of pane view window.

use ::derive_more::{Deref, Into, IsVariant};
use ::iced_core::{Function, window};
use ::iced_runtime::Task;
use ::iced_widget::{Column, Space, container, pane_grid, text};
use ::spel_katalog_assets as assets;
use ::spel_katalog_common::in_place::{Convene, MapSelf};
use ::spel_katalog_fold_with::Fold;
use ::spel_katalog_functional::LazyPipe;
use ::spel_katalog_list_menu::ListMenu;
use ::spel_katalog_widget::{Element, WidgetExt, icon};
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
    state: PaneState,
    is_maximized: bool,
) -> ListMenu<'a, crate::Message> {
    ListMenu::new()
        .title("Pane")
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
        .title("Header")
        .pipe(|menu| state.add_hide_show_buttons(id, pane, menu))
        .button(
            "Show All",
            Message::ShowAllTitlebars.then_pipe(crate::Message::PaneView.with(id)),
        )
        .button(
            "Hide All",
            Message::HideAllTitlebars.then_pipe(crate::Message::PaneView.with(id)),
        )
        .title("Split")
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
        .title("View")
        .fold_with(
            [
                PaneKind::Log,
                PaneKind::Terminal,
                PaneKind::Processes,
                PaneKind::Settings,
            ],
            move |list, pane_kind| {
                if pane_kind != *state {
                    list.button(
                        pane_kind.name(),
                        Message::SetState {
                            pane,
                            state: state.kind(pane_kind),
                        }
                        .then_pipe(crate::Message::PaneView.with(id)),
                    )
                } else {
                    list
                }
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
            pane_grid::Content::new(match state.kind {
                PaneKind::Empty => ::iced_aw::ContextMenu::new(
                    Space::new().pipe(::iced_widget::center),
                    move || context_menu(id, pane, *state, is_maximized).into(),
                )
                .pipe(Element::from),
                PaneKind::Log => self
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
                    .pipe(state.with_optional_spacing()),

                PaneKind::Terminal => self
                    .terminal
                    .view()
                    .map(crate::Message::Terminal)
                    .with_wrapper(::iced_aw::ContextMenu::new, move || {
                        context_menu(id, pane, *state, is_maximized).into()
                    })
                    .pipe(state.with_optional_spacing()),
                PaneKind::Settings => self
                    .settings
                    .view_scrollable()
                    .pipe(Element::from)
                    .map(crate::Message::Settings)
                    .with_wrapper(::iced_aw::ContextMenu::new, move || {
                        context_menu(id, pane, *state, is_maximized).into()
                    })
                    .pipe(state.with_optional_spacing()),
                PaneKind::Processes => self
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
                    .pipe(state.with_optional_spacing()),
                PaneKind::GameInfo => {
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
                            .pipe(state.with_optional_spacing())
                    } else {
                        container(text("No game is currently selected!"))
                            .style(::spel_katalog_widget::rounded_shadowed_box)
                            .pipe(::iced_widget::center)
                            .pipe(state.with_optional_spacing())
                    }
                }
            })
            .pipe(state.add_titlebar(id, pane, is_maximized))
        })
        .on_resize(5, move |event| {
            crate::Message::PaneView(id, Message::Resize(event))
        })
        .on_drag(move |event| crate::Message::PaneView(id, Message::Drag(event)))
        .spacing(5)
        .pipe(::iced_widget::container)
        .padding(5)
        .into()
    }
}

/// A pane view window/widget.
#[derive(Debug, Clone)]
pub struct State {
    /// Pane grid state.
    panes: pane_grid::State<PaneState>,
}

impl State {
    /// Create a state builder.
    pub fn builder(initial: PaneState) -> Builder {
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
                    (self.panes, _) = pane_grid::State::new(PaneKind::Empty.into());
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
                self.panes.split(axis, pane, PaneState::empty());
                Task::none()
            }
            Message::ShowAllTitlebars => {
                self.panes.panes.values_mut().for_each(|state| {
                    state.titlebar_ = true;
                });
                Task::none()
            }
            Message::HideAllTitlebars => {
                self.panes.panes.values_mut().for_each(|state| {
                    state.titlebar_ = false;
                });
                Task::none()
            }
            Message::ShowTitlebar(pane) => {
                if let Some(state) = self.panes.get_mut(pane) {
                    state.titlebar_ = true;
                }
                Task::none()
            }
            Message::HideTitlebar(pane) => {
                if let Some(state) = self.panes.get_mut(pane) {
                    state.titlebar_ = false;
                }
                Task::none()
            }
        }
    }

    /// Create new state.
    pub fn new(initial: PaneState) -> Self {
        Self::builder(initial).build()
    }
}

/// State builder for pane view.
#[derive(Debug, Clone)]
pub struct Builder {
    /// Last added pane.
    offset: pane_grid::Pane,
    /// Current state.
    panes: pane_grid::State<PaneState>,
}

impl Builder {
    /// Build state.
    pub fn build(self) -> State {
        let Self { offset: _, panes } = self;
        State { panes }
    }

    /// Perform a split across the given axis.
    pub fn split(self, pane: PaneState, axis: pane_grid::Axis) -> Self {
        let Self { offset, mut panes } = self;
        let offset = panes
            .split(axis, offset, pane)
            .map_or(offset, |(offset, _)| offset);
        Self { offset, panes }
    }

    /// Perform a horizontal split and add given pane.
    pub fn horizontal(self, pane: PaneState) -> Self {
        self.split(pane, pane_grid::Axis::Horizontal)
    }

    /// Perform a vertical split and add given pane.
    pub fn vertical(self, pane: PaneState) -> Self {
        self.split(pane, pane_grid::Axis::Vertical)
    }
}

/// State of a single pane.
#[derive(Debug, Clone, Copy, Deref, Into)]
pub struct PaneState {
    /// Kind of pane to show.
    #[deref]
    #[into]
    pub kind: PaneKind,
    /// Does the pane have a visible titlebar.
    titlebar_: bool,
}

impl PaneState {
    /// Construct new pane state.
    pub const fn new(kind: PaneKind) -> Self {
        Self {
            kind,
            titlebar_: false,
        }
    }

    /// Create new empty pane state.
    pub const fn empty() -> Self {
        Self::new(PaneKind::Empty)
    }

    /// Set kind for pane state.
    pub const fn kind(self, kind: PaneKind) -> Self {
        Self { kind, ..self }
    }

    /// Does the state have a titlebar.
    pub const fn has_titlebar(self) -> bool {
        if self.kind.forbids_titlebar() {
            false
        } else if self.kind.requires_titlebar() {
            true
        } else {
            self.titlebar_
        }
    }

    /// Create a function which adds a titlebar to some content if required.
    fn add_titlebar(
        self,
        id: window::Id,
        pane: pane_grid::Pane,
        is_maximized: bool,
    ) -> impl for<'a> Fn(pane_grid::Content<'a, crate::Message>) -> pane_grid::Content<'a, crate::Message>
    {
        move |content| {
            if !self.has_titlebar() {
                return content;
            }

            content
                .title_bar(
                    pane_grid::TitleBar::new(::iced_aw::ContextMenu::new(
                        text(self.name()),
                        move || context_menu(id, pane, self, is_maximized).into(),
                    ))
                    .padding(3)
                    .style(container::bordered_box)
                    .always_show_controls()
                    .controls(pane_grid::Controls::new(
                        ::iced_aw::ContextMenu::new(
                            ::iced_widget::Row::new()
                                .spacing(3)
                                .pipe_if(self.is_settings(), |row| {
                                    row.push(
                                        icon::Icon::new(assets::save())
                                            .size(16)
                                            .into_button()
                                            .on_press_with(|| {
                                                crate::Message::Settings(
                                                    ::spel_katalog_settings_view::Message::Save,
                                                )
                                            })
                                            .style(::iced_widget::button::success),
                                    )
                                })
                                .convene()
                                .push(if is_maximized {
                                    icon::Icon::new(assets::arrow_down())
                                        .size(16)
                                        .into_button()
                                        .style(::iced_widget::button::secondary)
                                        .on_press_with(
                                            Message::Restore
                                                .then_pipe(crate::Message::PaneView.with(id)),
                                        )
                                } else {
                                    icon::Icon::new(assets::arrow_up())
                                        .size(16)
                                        .into_button()
                                        .style(::iced_widget::button::secondary)
                                        .on_press_with(
                                            pane.then_pipe(Message::Maximize)
                                                .then_chain(crate::Message::PaneView.with(id)),
                                        )
                                })
                                .push(
                                    icon::Icon::new(assets::minimize())
                                        .size(16)
                                        .into_button()
                                        .style(::iced_widget::button::danger)
                                        .on_press_with(
                                            pane.then_pipe(Message::Close)
                                                .then_chain(crate::Message::PaneView.with(id)),
                                        ),
                                ),
                            move || context_menu(id, pane, self, is_maximized).into(),
                        ),
                    )),
                )
                .style(|theme| container::background(theme.palette().background))
        }
    }

    /// Add buttons to show/hide titlebar to list menu.
    fn add_hide_show_buttons<'a>(
        self,
        id: window::Id,
        pane: pane_grid::Pane,
        menu: ListMenu<'a, crate::Message>,
    ) -> ListMenu<'a, crate::Message> {
        // If state is required show no method of changing it.
        if self.forbids_titlebar() || self.requires_titlebar() {
            return menu;
        }

        if self.has_titlebar() {
            menu.button(
                "Hide",
                pane.then_pipe(Message::HideTitlebar)
                    .then_chain(crate::Message::PaneView.with(id)),
            )
        } else {
            menu.button(
                "Show",
                pane.then_pipe(Message::ShowTitlebar)
                    .then_chain(crate::Message::PaneView.with(id)),
            )
        }
    }

    /// Create function adding spacing and converting to an element.
    fn with_optional_spacing<'a, E>(self) -> impl Fn(E) -> Element<'a, crate::Message>
    where
        E: Into<Element<'a, crate::Message>>,
    {
        move |element| {
            if self.has_titlebar() {
                Column::new()
                    .push(Space::new().height(3))
                    .push(element)
                    .into()
            } else {
                element.into()
            }
        }
    }
}

impl From<PaneKind> for PaneState {
    fn from(value: PaneKind) -> Self {
        PaneState::new(value)
    }
}

/// Kind of displayed pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, IsVariant)]
pub enum PaneKind {
    /// Displau empty pane.
    #[default]
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

impl PaneKind {
    /// Get name of a pane.
    pub const fn name(self) -> &'static str {
        match self {
            PaneKind::Log => "Logs",
            PaneKind::Terminal => "Terminal",
            PaneKind::Settings => "Settings",
            PaneKind::Processes => "Processes",
            PaneKind::GameInfo => "Game Info",
            PaneKind::Empty => "Empty",
        }
    }

    /// Is the titlebar required for variant.
    pub const fn requires_titlebar(self) -> bool {
        matches!(
            self,
            PaneKind::GameInfo | PaneKind::Empty | PaneKind::Processes | PaneKind::Settings
        )
    }

    /// Is titlebar forbidden for variant.
    pub const fn forbids_titlebar(self) -> bool {
        false
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
    /// Show titlebars.
    ShowAllTitlebars,
    /// Hide titlebars.
    HideAllTitlebars,
    /// Show titlebar for pane.
    ShowTitlebar(pane_grid::Pane),
    /// Hide titlebar for pane.
    HideTitlebar(pane_grid::Pane),
    /// Set state of pane.
    SetState {
        /// Pane to set state of.
        pane: pane_grid::Pane,
        /// State to set pane to.
        state: PaneState,
    },
}
