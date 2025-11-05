use ::std::ops::{Index, IndexMut};

#[derive(Debug, Clone, Default)]
pub struct DocsState {
    entries: Vec<bool>,
}

impl DocsState {
    pub fn new_id(&mut self) -> ItemId {
        let id = self.entries.len();
        self.entries.push(true);
        ItemId(id)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemId(usize);
