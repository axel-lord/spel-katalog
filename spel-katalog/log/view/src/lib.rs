//! View of log.

use ::bytes::Bytes;
use ::derive_more::{Deref, DerefMut};
use ::iced_core::{Alignment::Center, Border, Element, Font, Length::Fill, Theme};
use ::iced_runtime::{Task, futures::Subscription};
use ::iced_widget as widget;
use ::smol::stream::StreamExt;
use ::spel_katalog_log::{OwnedRecord, RecordMessage};
use ::tap::Pipe;

/// Message used by [LogView].
#[derive(Debug, Clone)]
pub enum Message {
    /// Receive a log record.
    RecvRecord(RecordMessage),
    /// Toggle log view.
    Toggle(usize),
}

/// Content to display for a record.
#[derive(Debug, Clone, Deref, DerefMut)]
enum RecordContent {
    /// Record is closed.
    Closed {
        /// Record of content.
        #[deref]
        #[deref_mut]
        record: OwnedRecord,
        /// Short representation of record.
        short: Bytes,
    },
    /// Record is open.
    Open {
        /// Record of content.
        #[deref]
        #[deref_mut]
        record: OwnedRecord,
    },
}

impl RecordContent {
    /// Create open variant from a record.
    pub const fn new_open(record: OwnedRecord) -> Self {
        Self::Open { record }
    }

    /// Create closed variant from a record.
    pub fn new_closed(record: OwnedRecord) -> Self {
        Self::Closed {
            short: record.first_line(),
            record,
        }
    }

    /// Convert to [RecordContent::Open].
    pub fn open(&mut self) {
        if let Self::Closed { record, .. } = self {
            *self = Self::new_open(record.clone())
        }
    }

    /// Convert to [RecordContent::Closed].
    pub fn close(&mut self) {
        if let Self::Open { record, .. } = self {
            *self = Self::new_closed(record.clone())
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
            RecordContent::Closed { record, short } => short.len() != record.message.len(),
            RecordContent::Open { .. } => true,
        }
    }
}

/// Log view.
#[derive(Debug, Default)]
pub struct LogView {
    /// Stored records.
    records: Vec<RecordContent>,
}

impl LogView {
    /// Get subscription of log view.
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run(|| {
            ::spel_katalog_log::receiver()
                .clone()
                .into_stream()
                .map(Message::RecvRecord)
        })
    }

    /// Update widget state.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RecvRecord(record) => {
                self.records.push(RecordContent::new_closed(record.into()));
                Task::none()
            }
            Message::Toggle(idx) => {
                if let Some(record) = self.records.get_mut(idx) {
                    record.toggle();
                }
                Task::none()
            }
        }
    }

    /// View widget.
    pub fn view(&self) -> Element<'_, Message, ::iced_core::Theme, ::iced_widget::Renderer> {
        fn badge<'a>(level: ::log::Level) -> widget::Container<'a, Message> {
            widget::text(match level {
                ::log::Level::Error => "Error",
                ::log::Level::Warn => "Warning",
                ::log::Level::Info => "Info",
                ::log::Level::Debug => "Debug",
                ::log::Level::Trace => "Trace",
            })
            .font(Font::MONOSPACE)
            .pipe(widget::container)
            .padding(3)
            .style(move |theme: &Theme| {
                let palette = theme.palette();
                widget::container::rounded_box(theme).border(
                    Border::default()
                        .color(match level {
                            ::log::Level::Error => palette.danger,
                            ::log::Level::Warn => palette.warning,
                            ::log::Level::Info => palette.success,
                            ::log::Level::Debug => palette.primary,
                            ::log::Level::Trace => palette.background,
                        })
                        .width(1.5)
                        .rounded(3),
                )
            })
        }
        self.records
            .iter()
            .enumerate()
            .fold(widget::Column::new(), |col, (idx, record)| {
                if record.is_shortened() {
                    match record {
                        RecordContent::Closed { record, short } => col.push(
                            widget::Row::new()
                                .align_y(Center)
                                .spacing(3)
                                .push(widget::text("+").font(Font::MONOSPACE))
                                .push(badge(record.level))
                                .push(
                                    String::from_utf8_lossy(short)
                                        .pipe(widget::text)
                                        .font(Font::MONOSPACE)
                                        .pipe(widget::container)
                                        .padding(3),
                                )
                                .pipe(widget::button)
                                .style(widget::button::text)
                                .padding(0)
                                .on_press(Message::Toggle(idx)),
                        ),
                        RecordContent::Open { record } => col.push(
                            widget::Row::new()
                                .spacing(3)
                                .push(
                                    widget::Row::new()
                                        .spacing(3)
                                        .align_y(Center)
                                        .push(widget::text("-").font(Font::MONOSPACE))
                                        .push(badge(record.level)),
                                )
                                .push(
                                    String::from_utf8_lossy(&record.message)
                                        .pipe(widget::text)
                                        .font(Font::MONOSPACE)
                                        .pipe(widget::container)
                                        .style(widget::container::rounded_box)
                                        .padding(3),
                                )
                                .pipe(widget::button)
                                .style(widget::button::text)
                                .padding(0)
                                .on_press(Message::Toggle(idx)),
                        ),
                    }
                } else {
                    col.push(
                        widget::Row::new()
                            .align_y(Center)
                            .spacing(3)
                            .push(badge(record.level))
                            .push(
                                String::from_utf8_lossy(&record.message)
                                    .pipe(widget::text)
                                    .font(Font::MONOSPACE),
                            ),
                    )
                }
            })
            .spacing(3)
            .pipe(::spel_katalog_widget::xy_scrollable)
            .width(Fill)
            .pipe(widget::container)
            .style(widget::container::bordered_box)
            .padding(3)
            .height(Fill)
            .into()
    }
}
