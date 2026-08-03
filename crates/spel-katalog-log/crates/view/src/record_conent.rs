//! [RecordContent] impl.

use ::bytes::Bytes;
use ::derive_more::{Deref, DerefMut};
use ::spel_katalog_log::OwnedRecord;

use crate::record::Record;

/// Content to display for a record.
#[derive(Debug, Clone, Deref, DerefMut)]
pub enum RecordContent {
    /// Record is closed.
    Closed {
        /// Record of content.
        #[deref]
        #[deref_mut]
        record: Record,
        /// Short representation of record.
        short: Bytes,
    },
    /// Record is open.
    Open {
        /// Record of content.
        #[deref]
        #[deref_mut]
        record: Record,
    },
}

impl RecordContent {
    /// Create closed variant from a record.
    pub fn new(record: OwnedRecord) -> Self {
        Self::Closed {
            short: record.first_line(),
            record: Record::new(record),
        }
    }

    /// Convert to [RecordContent::Open].
    pub fn open(&mut self) {
        if let Self::Closed { record, .. } = self {
            *self = Self::Open {
                record: record.clone(),
            }
        }
    }

    /// Convert to [RecordContent::Closed].
    pub fn close(&mut self) {
        if let Self::Open { record, .. } = self {
            *self = Self::Closed {
                record: record.clone(),
                short: record.first_line(),
            }
        }
    }

    /// Toggle state.
    pub fn toggle(&mut self) {
        match self {
            Self::Open { .. } => self.close(),
            Self::Closed { .. } => self.open(),
        }
    }

    /// Is shortened.
    pub const fn is_shortened(&self) -> bool {
        match self {
            RecordContent::Closed { record, short } => {
                short.len() != record.as_owned_record().message.len()
            }
            RecordContent::Open { .. } => true,
        }
    }
}
