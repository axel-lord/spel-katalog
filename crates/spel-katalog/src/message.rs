//! Message definitions.
#![allow(clippy::missing_docs_in_private_items, reason = "todo")]

use ::derive_more::{From, IsVariant};
use ::iced_core::window;
use ::spel_katalog_common::OrRequest;
use ::spel_katalog_formats::NativeGameConfig;

use crate::{app::WindowType, pane_view, view};

/// Safety to use when running games.
#[derive(Debug, Clone, Copy, Default, IsVariant, PartialEq, Eq, Hash)]
pub enum Safety {
    /// Use no sandboxing.
    None,
    /// Sandbox game.
    #[default]
    Sandbox,
    /// Run a sandboxed shell.
    SandboxShell,
}

impl From<bool> for Safety {
    fn from(value: bool) -> Self {
        if value { Self::Sandbox } else { Self::None }
    }
}

/// Small message which may be copied.
#[derive(Debug, IsVariant, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuickMessage {
    CloseAll,
    ClosePane,
    CycleFilter,
    CycleHidden,
    Next,
    OpenProcessInfo,
    OpenGameInfo,
    Prev,
    RunSelected,
    ToggleGameInfo,
    ToggleMain,
    ToggleNetwork,
    ToggleProcessInfo,
    ToggleSettings,
    Debug,
    ConvertAll,
    OpenDatabase,
    CopyFilter,
    PasteFilter,
    ReloadGames,
    OpenInstaller,
    ShowWelcome,
    ShowTagFilter,
    EscapeOne,
}

#[derive(Debug, IsVariant, From, Clone)]
pub enum Message {
    #[from]
    Status(String),
    Filter(String),
    #[from]
    Settings(::spel_katalog_settings_view::Message),
    #[from]
    View(view::Message),
    #[from]
    Games(OrRequest<::spel_katalog_games::Message, ::spel_katalog_games::Request>),
    #[from]
    Info(OrRequest<::spel_katalog_info::Message, ::spel_katalog_info::Request>),
    #[from]
    Quick(QuickMessage),
    ViewProcess {
        /// Pid of process to add to view set.
        pid: i64,
    },
    OpenWindow(window::Id, WindowType),
    CloseWindow(window::Id),
    Installer(
        window::Id,
        OrRequest<::spel_katalog_installer::Message, ::spel_katalog_installer::Request>,
    ),
    #[from]
    Terminal(::spel_katalog_terminal::Message),
    #[from]
    ShowInfo(crate::view::Displayed),
    #[from]
    Ipc(::spel_katalog_formats::InstallerConfig),
    RunGameNative(Box<NativeGameConfig>),
    RunShellNative(Box<NativeGameConfig>),
    #[from]
    TagFilter(OrRequest<::spel_katalog_tag_filter::Message, ::spel_katalog_tag_filter::Request>),
    #[from]
    ProcessView(::spel_katalog_process_view::Message),
    PaneView(window::Id, pane_view::Message),
}

impl<T, E> From<Result<T, E>> for Message
where
    T: Into<Message>,
    E: Into<Message>,
{
    fn from(value: Result<T, E>) -> Self {
        match value {
            Ok(value) => value.into(),
            Err(value) => value.into(),
        }
    }
}

impl From<::color_eyre::Report> for Message {
    fn from(value: ::color_eyre::Report) -> Self {
        value.to_string().into()
    }
}

impl From<::spel_katalog_info::Message> for Message {
    fn from(message: ::spel_katalog_info::Message) -> Self {
        Self::Info(OrRequest::Message(message))
    }
}
