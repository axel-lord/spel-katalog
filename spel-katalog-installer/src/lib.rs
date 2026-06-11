//! Utility to install new native games.

use ::iced_runtime::Task;
use ::iced_widget as widget;

/// Message used by installer.
#[derive(Debug, Clone)]
pub enum Message {}

/// Installer window for a game.
#[derive(Debug, Clone, Default)]
pub struct Installer {}

impl Installer {
    /// Update application state using message.
    pub const fn update(&mut self, message: Message) -> Task<Message> {
        match message {}
    }

    /// View installer state.
    pub fn view(
        &self,
    ) -> ::iced_core::Element<'_, Message, ::iced_core::Theme, ::iced_widget::Renderer> {
        widget::text("Placeholder!").into()
    }
}
