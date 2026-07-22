//! [Record] impl

use ::core::cell::OnceCell;

use ::derive_more::Deref;
use ::spel_katalog_formats::Timestamp;
use ::spel_katalog_log::OwnedRecord;

/// A stored record. Wrapped to add functions
/// for conveniance, and disallow mutation.
#[derive(Debug, Clone, Deref)]
pub struct Record {
    /// Wrapped owned record.
    #[deref]
    inner: OwnedRecord,

    /// Cache used to display record timestamp.
    timestamp_cache: OnceCell<String>,

    /// Cache used to display record clock time.
    clock_cache: OnceCell<String>,
}

impl Record {
    /// Get inner owned record.
    pub const fn as_owned_record(&self) -> &OwnedRecord {
        &self.inner
    }

    /// Construct a new record.
    pub const fn new(owned_record: OwnedRecord) -> Self {
        Self {
            inner: owned_record,
            timestamp_cache: OnceCell::new(),
            clock_cache: OnceCell::new(),
        }
    }

    /// Get a timestamp string.
    pub fn timestamp_str(&self) -> &str {
        self.timestamp_cache
            .get_or_init(|| self.inner.timestamp.to_string())
    }

    /// Get a clock string.
    pub fn clock_str(&self) -> &str {
        self.clock_cache
            .get_or_init(|| Timestamp::clock(&self.timestamp))
    }
}

impl From<OwnedRecord> for Record {
    fn from(value: OwnedRecord) -> Self {
        Self::new(value)
    }
}
