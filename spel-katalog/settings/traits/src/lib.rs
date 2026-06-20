//! Traits in use by settings.

pub use ::spel_katalog_settings_derive::*;

/// Trait to provide a default string representation of a type.
pub trait DefaultStr {
    /// Get the default string representation of self.
    fn default_str() -> &'static str;
}

/// Trait to provide titles for settings
pub trait Title {
    /// Title to use for setting.
    fn title() -> &'static str;
}

/// Trait to provide help for settings.
pub trait Help {
    /// Get help for setting.
    fn help() -> &'static str;
}

/// Trait for types which index settings.
pub trait SettingsIndex<S> {
    /// Output type returned by indexing
    type Output: ?Sized;

    /// Get the output type.
    fn get(self, settings: &S) -> &Self::Output;
}

/// Trait for types wich index settings.
pub trait SettingsIndexMut<S>
where
    Self: SettingsIndex<S>,
{
    /// Get the output type as mutable.
    fn get_mut(self, settings: &mut S) -> &mut Self::Output;
}

/// Trait for types which may supply an index type.
pub trait AsIndex<S> {
    /// Output type of index operation.
    type Output;
    /// Supply the index.
    fn as_idx() -> impl SettingsIndexMut<S, Output = Self::Output>;
}

/// Trait for simple enums to provide all values.
///
/// # Safety
/// The `VARIANTS` associated constant must contain all variants.
pub unsafe trait Variants
where
    Self: 'static + Sized,
{
    /// All values for enum.
    const VARIANTS: &[Self];

    /// Select the next variant.
    fn cycle(&self) -> Self
    where
        Self: PartialEq + Clone,
    {
        let idx = Self::VARIANTS
            .iter()
            .position(|v| v == self)
            .unwrap_or_else(|| unreachable!());
        Self::VARIANTS[(idx + 1) % Self::VARIANTS.len()].clone()
    }
}
