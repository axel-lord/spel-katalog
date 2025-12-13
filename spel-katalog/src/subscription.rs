use ::std::time::Duration;

use ::iced_core::keyboard::{self, Modifiers, key::Named};
use ::iced_futures::Subscription;
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

        let key_event = ::iced::keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed {
                key,
                modified_key: _,
                physical_key: _,
                location: _,
                modifiers,
                text: _,
                repeat: _,
            } => Some(Message::Quick(if modifiers.is_empty() {
                match key.as_ref() {
                    keyboard::Key::Character(chr) => match chr {
                        "q" => QuickMessage::ClosePane,
                        "h" => QuickMessage::CycleHidden,
                        "f" => QuickMessage::CycleFilter,
                        "n" => QuickMessage::ToggleNetwork,
                        _ => return None,
                    },
                    keyboard::Key::Named(named) => match named {
                        Named::ArrowRight if modifiers.is_empty() => return sel(SelDir::Right),
                        Named::ArrowLeft if modifiers.is_empty() => return sel(SelDir::Left),
                        Named::ArrowUp if modifiers.is_empty() => return sel(SelDir::Up),
                        Named::ArrowDown if modifiers.is_empty() => return sel(SelDir::Down),

                        Named::Tab => QuickMessage::Next,
                        Named::Enter | Named::Space => QuickMessage::RunSelected,

                        Named::F1 => QuickMessage::ToggleLuaApi,
                        Named::F2 => QuickMessage::ToggleSettings,
                        Named::F3 => QuickMessage::ToggleMain,
                        Named::F5 => QuickMessage::ToggleGameInfo,
                        Named::F6 => QuickMessage::ToggleBatch,
                        Named::F7 => QuickMessage::ToggleProcessInfo,
                        _ => return None,
                    },
                    _ => return None,
                }
            } else if modifiers == Modifiers::SHIFT | Modifiers::CTRL {
                let keyboard::Key::Character(chr) = key.as_ref() else {
                    return None;
                };
                match chr {
                    "b" => QuickMessage::ToggleBatch,
                    "m" => QuickMessage::ToggleMain,
                    "s" => QuickMessage::ToggleSettings,
                    "l" => QuickMessage::ToggleLuaApi,
                    "p" => QuickMessage::ToggleProcessInfo,
                    "g" => QuickMessage::ToggleGameInfo,
                    _ => return None,
                }
            } else if modifiers == Modifiers::SHIFT {
                let keyboard::Key::Named(Named::Tab) = key else {
                    return None;
                };
                QuickMessage::Prev
            } else if modifiers == Modifiers::CTRL {
                if let keyboard::Key::Character(chr) = key.as_ref()
                    && chr == "q"
                {
                    QuickMessage::CloseAll
                } else {
                    return None;
                }
            } else {
                return None;
            })),
            _ => None,
        });

        let refresh = if self.view.displayed.is_processes() {
            ::iced_futures::backend::default::time::every(Duration::from_millis(500))
                .map(|_| Message::Quick(QuickMessage::RefreshProcessInfo))
        } else {
            Subscription::none()
        };

        let window_close = ::iced_runtime::window::close_events().map(Message::CloseWindow);
        let games = self
            .games
            .subscription()
            .map(OrRequest::Message)
            .map(Message::Games);

        Subscription::batch([key_event, window_close, refresh, games])
    }
}
