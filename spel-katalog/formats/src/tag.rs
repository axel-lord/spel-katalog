//! [Tag] and [TagId] impls.

use ::core::{borrow::Borrow, num::NonZero, sync::atomic::AtomicU64};
use ::std::sync::Arc;

use ::dashmap::DashMap;
use ::derive_more::{Deref, DerefMut, Display, From, Into, IsVariant};
use ::rustc_hash::FxBuildHasher;
use ::serde::{Deserialize, Serialize};

/// Id of a tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct TagId {
    /// Numeric id of tag.
    id: NonZero<u64>,
}

impl TagId {
    /// Construct a new tag id.
    #[expect(clippy::missing_panics_doc, reason = "will not panic")]
    #[expect(
        clippy::new_without_default,
        reason = "new implementation does not fit as default, and type should not have a default value"
    )]
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);

        Self {
            id: NonZero::new(COUNTER.fetch_add(1, ::core::sync::atomic::Ordering::Relaxed))
                .expect("function scope static atomic counter initialized tp 1 should never be 0"),
        }
    }
}

/// A game tag/category.
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    From,
    Into,
    Deref,
)]
#[repr(transparent)]
#[serde(transparent)]
#[deref(forward)]
pub struct Tag {
    /// Name of the tag.
    pub name: String,
}

impl Borrow<str> for Tag {
    fn borrow(&self) -> &str {
        self
    }
}

impl Tag {
    /// Construct a new tag with the given name.
    pub const fn new(name: String) -> Self {
        Tag { name }
    }
}

/// A Filter to apply to tags.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Deref, DerefMut,
)]
pub struct TagFilter<T> {
    /// What kind of filter is this.
    pub kind: TagFilterKind,
    /// Mode of the filter.
    pub mode: TagFilterMode,
    /// Tags used by the filter.
    #[deref]
    #[deref_mut]
    pub tags: T,
}

impl<T> TagFilter<T> {
    /// Map tag storage.
    pub fn map<F, V>(this: TagFilter<T>, f: F) -> TagFilter<V>
    where
        F: FnOnce(T) -> V,
    {
        let Self { kind, mode, tags } = this;
        TagFilter {
            kind,
            mode,
            tags: f(tags),
        }
    }
}

/// What should the filter do.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    IsVariant,
    Display,
)]
pub enum TagFilterKind {
    /// On filter match include value.
    Include,
    /// On filter match exclude value.
    Exclude,
}

/// How the filter tags match values.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    IsVariant,
    Display,
)]
pub enum TagFilterMode {
    /// Filter is a match if any tag is present.
    Any,
    /// Filter is a match if and onlt if all tags are present.
    All,
}

/// Wrapper for tag storage.
#[derive(Debug, Clone, Default, Deref)]
#[deref(forward)]
pub struct TagStorage {
    /// Wrapped tag storage.
    inner: Arc<DashMap<Tag, TagId, FxBuildHasher>>,
}

impl TagStorage {
    /// Get the id of a tag.
    pub fn get_id(&self, tag: &str) -> TagId {
        if let Some(id) = self.get(tag) {
            *id
        } else {
            *self
                .entry(Tag {
                    name: tag.to_owned(),
                })
                .or_insert_with(TagId::new)
        }
    }
}
