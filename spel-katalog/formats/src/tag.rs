//! [Tag] and [TagId] impls.

use ::core::{num::NonZero, sync::atomic::AtomicU64};

use ::derive_more::{Deref, From, Into};
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

impl Tag {
    /// Construct a new tag with the given name.
    pub const fn new(name: String) -> Self {
        Tag { name }
    }
}
