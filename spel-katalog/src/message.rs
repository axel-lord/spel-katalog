use ::derive_more::{From, IsVariant};
use ::iced::window;
use ::spel_katalog_common::OrRequest;

use crate::{
    app::WindowType,
    dialog::{self, DialogBuilder},
    process_info, view,
};

#[derive(Debug, Clone, Copy, Default, IsVariant, PartialEq, Eq, Hash)]
pub enum Safety {
    None,
    #[default]
    Firejail,
}

impl From<bool> for Safety {
    fn from(value: bool) -> Self {
        if value { Self::Firejail } else { Self::None }
    }
}

#[derive(Debug, IsVariant, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum QuickMessage {
    ClosePane,
    CloseAll,
    ToggleSettings,
    OpenProcessInfo,
    CycleHidden,
    CycleFilter,
    ToggleNetwork,
    RefreshProcessInfo,
    RunSelected,
    Next,
    Prev,
    ToggleBatch,
    ToggleLuaApi,
    ShowMain,
}

#[derive(Debug, IsVariant, From)]
pub enum Message {
    #[from]
    Status(String),
    Filter(String),
    #[from]
    Settings(::spel_katalog_settings::Message),
    #[from]
    View(view::Message),
    #[from]
    Games(OrRequest<::spel_katalog_games::Message, ::spel_katalog_games::Request>),
    #[from]
    Info(OrRequest<::spel_katalog_info::Message, ::spel_katalog_info::Request>),
    #[from]
    Quick(QuickMessage),
    ProcessInfo(Option<Vec<process_info::ProcessInfo>>),
    Kill {
        pid: i64,
        terminate: bool,
    },
    #[from]
    Batch(OrRequest<::spel_katalog_batch::Message, ::spel_katalog_batch::Request>),
    OpenWindow(window::Id, WindowType),
    CloseWindow(window::Id),
    DialogRequest(window::Id, dialog::Request),
    DialogMessage(window::Id, dialog::Message),
    Dialog(DialogBuilder),
    #[from]
    Terminal(::spel_katalog_terminal::Message),
    LuaDocs(window::Id, ::spel_katalog_lua_docs::Message),
}
