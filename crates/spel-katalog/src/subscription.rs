use ::iced_core::keyboard::{self, Modifiers, key::Named};
use ::iced_futures::Subscription;
use ::spel_katalog_common::OrRequest;
use ::spel_katalog_games::SelDir;
use ::tap::Pipe;

use crate::{App, Message, QuickMessage};

fn sel(sel_dir: SelDir) -> Option<Message> {
    sel_dir
        .pipe(::spel_katalog_games::Message::Select)
        .pipe(OrRequest::Message)
        .pipe(Message::Games)
        .pipe(Some)
}

fn unmodified_chr_key(chr: &str) -> Option<Message> {
    Some(Message::Quick(match chr {
        "q" => QuickMessage::ClosePane,
        "h" => QuickMessage::CycleHidden,
        "f" => QuickMessage::CycleFilter,
        "n" => QuickMessage::ToggleNetwork,
        _ => return None,
    }))
}

fn unmodified_named_key(named: Named) -> Option<Message> {
    Some(Message::Quick(match named {
        Named::ArrowRight => return sel(SelDir::Right),
        Named::ArrowLeft => return sel(SelDir::Left),
        Named::ArrowUp => return sel(SelDir::Up),
        Named::ArrowDown => return sel(SelDir::Down),

        Named::Tab => QuickMessage::Next,
        Named::Enter | Named::Space => QuickMessage::RunSelected,
        Named::F2 => QuickMessage::ToggleSettings,
        Named::F3 => QuickMessage::ToggleMain,
        Named::Escape => QuickMessage::EscapeOne,
        _ => return None,
    }))
}

const fn shift_named_key(named: Named) -> Option<Message> {
    Some(Message::Quick(match named {
        Named::Tab => QuickMessage::Prev,
        _ => return None,
    }))
}

fn ctrl_chr_key(chr: &str) -> Option<Message> {
    Some(Message::Quick(match chr {
        "q" => QuickMessage::CloseAll,
        _ => return None,
    }))
}

fn ctrl_shift_chr_key(chr: &str) -> Option<Message> {
    Some(Message::Quick(match chr {
        "m" => QuickMessage::ToggleMain,
        "s" => QuickMessage::ToggleSettings,
        "d" => QuickMessage::Debug,
        "w" => QuickMessage::ShowWelcome,
        "t" => QuickMessage::ShowTagFilter,
        _ => return None,
    }))
}

fn key_to_message(key: keyboard::Key<&str>, modifiers: Modifiers) -> Option<Message> {
    match key {
        keyboard::Key::Named(named) if modifiers.is_empty() => unmodified_named_key(named),
        keyboard::Key::Named(named) if modifiers == Modifiers::SHIFT => shift_named_key(named),
        keyboard::Key::Character(chr) if modifiers.is_empty() => unmodified_chr_key(chr),
        keyboard::Key::Character(chr) if modifiers == Modifiers::CTRL => ctrl_chr_key(chr),
        keyboard::Key::Character(chr) if modifiers == Modifiers::CTRL | Modifiers::SHIFT => {
            ctrl_shift_chr_key(chr)
        }
        _ => None,
    }
}

impl App {
    pub fn subscription(&self) -> Subscription<Message> {
        let key_event = ::iced_futures::keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed {
                key,
                modified_key: _,
                physical_key: _,
                location: _,
                modifiers,
                text: _,
                repeat: _,
            } => key_to_message(key.as_ref(), modifiers),
            _ => None,
        });

        let refresh = self.process_view.subscription().map(Message::ProcessView);

        let window_close = ::iced_runtime::window::close_events().map(Message::CloseWindow);
        let games = self
            .games
            .subscription()
            .map(OrRequest::Message)
            .map(Message::Games);

        let log_view = self.log_view.subscription().map(Message::LogView);

        Subscription::batch([key_event, window_close, refresh, games, log_view])
    }
}
