//! Utilities for changing structs.

/// Provide a type representing a change to implementor.
pub trait Delta {
    /// Delta type.
    type Delta;

    /// Apply change to self.
    fn apply(&mut self, delta: Self::Delta);
}

/// Trait for types where  
pub trait DeltaCmp: Delta {
    /// List of changes.
    type ChangeBuf: IntoIterator<Item = Self::Delta>;

    /// Find fields of other which differ from self.
    /// First item should be values in self, and second in other.
    fn delta(&self, other: &Self) -> [Self::ChangeBuf; 2];
}
