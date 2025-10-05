use ::derive_more::{From, IsVariant};
use ::spel_katalog_common::OrRequest;

use crate::{Safety, process_info, view};

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
}

#[derive(Debug, IsVariant, From, Clone)]
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
    RunGame(i64, Safety),
    #[from]
    Quick(QuickMessage),
    ProcessInfo(Option<Vec<process_info::ProcessInfo>>),
    Kill(i64),
    #[from]
    Batch(OrRequest<::spel_katalog_batch::Message, ::spel_katalog_batch::Request>),
}
