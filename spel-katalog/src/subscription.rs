use ::std::time::Duration;

use ::iced::{
    Subscription,
    keyboard::{self, Modifiers, key::Named, on_key_press},
    window,
};
use ::spel_katalog_common::OrRequest;
use ::spel_katalog_games::SelDir;
use ::tap::Pipe;

use crate::{App, Message, QuickMessage};

impl App {
    pub fn subscription(&self) -> Subscription<Message> {
        fn sel(sel_dir: SelDir) -> Option<Message> {
            sel_dir
                .pipe(::spel_katalog_games::Message::Select)
                .pipe(OrRequest::Message)
                .pipe(Message::Games)
                .pipe(Some)
        }
        let on_key = on_key_press(|key, modifiers| match key.as_ref() {
            keyboard::Key::Named(named) => match named {
                Named::Tab if modifiers.is_empty() => Some(QuickMessage::Next).map(Message::Quick),
                Named::Tab if modifiers == Modifiers::SHIFT => {
                    Some(QuickMessage::Prev).map(Message::Quick)
                }
                Named::ArrowRight if modifiers.is_empty() => sel(SelDir::Right),
                Named::ArrowLeft if modifiers.is_empty() => sel(SelDir::Left),
                Named::ArrowUp if modifiers.is_empty() => sel(SelDir::Up),
                Named::ArrowDown if modifiers.is_empty() => sel(SelDir::Down),
                Named::Enter | Named::Space if modifiers.is_empty() => {
                    Some(Message::Quick(QuickMessage::RunSelected))
                }
                Named::F1 => Some(Message::Quick(QuickMessage::ToggleLuaApi)),
                _ => None,
            },
            keyboard::Key::Character(chr) => match chr {
                "q" if modifiers.is_empty() => Some(QuickMessage::ClosePane),
                "q" if modifiers == Modifiers::CTRL => Some(QuickMessage::CloseAll),
                "h" if modifiers.is_empty() => Some(QuickMessage::CycleHidden),
                "f" if modifiers.is_empty() => Some(QuickMessage::CycleFilter),
                "s" if modifiers.is_empty() => Some(QuickMessage::ToggleSettings),
                "n" if modifiers.is_empty() => Some(QuickMessage::ToggleNetwork),
                "k" if modifiers == Modifiers::CTRL | Modifiers::SHIFT => {
                    Some(QuickMessage::OpenProcessInfo)
                }
                "b" if modifiers == Modifiers::CTRL | Modifiers::SHIFT => {
                    Some(QuickMessage::ToggleBatch)
                }
                "m" if modifiers == Modifiers::CTRL | Modifiers::SHIFT => {
                    Some(QuickMessage::ShowMain)
                }
                _ => None,
            }
            .map(Message::Quick),
            keyboard::Key::Unidentified => None,
        });

        let refresh = if self.process_list.is_some() {
            ::iced::time::every(Duration::from_millis(500))
                .map(|_| Message::Quick(QuickMessage::RefreshProcessInfo))
        } else {
            Subscription::none()
        };

        let window_close = window::close_events().map(Message::CloseWindow);
        let games = self
            .games
            .subscription()
            .map(OrRequest::Message)
            .map(Message::Games);

        Subscription::batch([on_key, window_close, refresh, games])
    }
}
