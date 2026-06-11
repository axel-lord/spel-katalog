//! Game management utilities.

mod games;
mod state;

pub use games::{GameAddDelta, Games, RemoveGames, WithThumb as GameWithThumb};
pub use state::{Message, Request, SelDir, State};

/// Element alias.
type Element<'a, M> = ::iced_core::Element<'a, M, ::iced_core::Theme, ::iced_renderer::Renderer>;
