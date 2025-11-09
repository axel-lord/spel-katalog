//! State of documentation implementation.

use ::std::ops::{Index, IndexMut};

/// Documentation view state.
#[derive(Debug, Clone, Default)]
pub struct DocsState {
    /// Display state of items.
    entries: Vec<bool>,
}

impl DocsState {
    /// Create a new entry and get it's id.
    pub fn new_id(&mut self) -> ItemId {
        let id = self.entries.len();
        self.entries.push(true);
        ItemId(id)
    }

    /// Set display state of all entries.
    pub fn set_all(&mut self, value: bool) {
        self.entries.iter_mut().for_each(|entry| *entry = value);
    }
}

impl Index<ItemId> for DocsState {
    type Output = bool;

    fn index(&self, index: ItemId) -> &Self::Output {
        &self.entries[index.0]
    }
}

impl IndexMut<ItemId> for DocsState {
    fn index_mut(&mut self, index: ItemId) -> &mut Self::Output {
        &mut self.entries[index.0]
    }
}

/// Id of a displayed item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemId(usize);
