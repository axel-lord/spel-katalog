//! Table of game info

use ::spel_katalog_formats::GameId;
use ::tabled::{
    Table,
    settings::{Alignment, Style, object::Columns},
};

/// Table of game info.
#[derive(Debug, Clone, Copy)]
pub struct InfoTable {
    /// Id of game.
    pub id: GameId,
    /// Shadowed game.
    pub shadows: Option<GameId>,
}

impl InfoTable {
    /// Get a [Table] of values.
    pub fn get_table(self) -> Table {
        let Self { id, shadows } = self;
        let mut builder = ::tabled::builder::Builder::new();

        builder.push_record(["UUID".to_owned(), id.to_string()]);

        if let Some(shadows) = shadows {
            builder.push_record(["Shadows".to_owned(), shadows.to_string()]);
        }

        let mut table = builder.build();

        table.modify(Columns::first(), Alignment::right());
        table.with(Style::empty());

        table
    }
}
