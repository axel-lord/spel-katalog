//! View of log.

use ::bytes::Bytes;
use ::derive_more::{Deref, DerefMut};
use ::iced_core::{
    Alignment::Center, Border, Element, Font, Length::Fill, Theme, text::IntoFragment,
};
use ::iced_runtime::{Task, futures::Subscription};
use ::iced_widget::{self as widget};
use ::smol::stream::StreamExt;
use ::spel_katalog_assets as assets;
use ::spel_katalog_log::{OwnedRecord, RecordMessage};
use ::spel_katalog_widget::{WithTooltip, icon};
use ::tap::Pipe;

use crate::record::Record;

mod record;

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

/// Log view.
#[derive(Debug)]
pub struct LogView {
    /// Stored records.
    records: Vec<RecordContent>,
    /// Font size of view.
    font_size: u32,
}

impl Default for LogView {
    fn default() -> Self {
        Self {
            records: Default::default(),
            font_size: 13,
        }
    }
}

/// Calculate icon size from font size.
fn icon_size(font_size: u32) -> u32 {
    font_size
        .checked_mul(3)
        .and_then(|size| size.checked_div(4))
        .unwrap_or(4)
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
                self.records.push(RecordContent::new(record.into()));
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

    /// Create a container with monospace text of
    /// standardized padding and font size
    fn text_container<'a>(
        &self,
        text: impl 'a + IntoFragment<'a>,
    ) -> widget::Container<'a, Message> {
        widget::text(text)
            .font(Font::MONOSPACE)
            .size(self.font_size)
            .pipe(widget::container)
            .padding(3)
    }

    /// Crate a box with monospace text of
    /// standardized padding and font size
    fn text_box<'a>(&self, text: impl 'a + IntoFragment<'a>) -> widget::Container<'a, Message> {
        self.text_container(text)
            .style(widget::container::rounded_box)
    }

    /// View a log badge.
    fn badge<'a>(&'a self, record: &'a Record) -> widget::Container<'a, Message> {
        self.text_container(match record.level {
            ::log::Level::Error => "Error",
            ::log::Level::Warn => "Warning",
            ::log::Level::Info => "Info",
            ::log::Level::Debug => "Debug",
            ::log::Level::Trace => "Trace",
        })
        .style(move |theme: &Theme| {
            let palette = theme.palette();
            widget::container::rounded_box(theme).border(
                Border::default()
                    .color(match record.level {
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

    /// View module text.
    fn module<'a>(&'a self, record: &'a Record) -> widget::Container<'a, Message> {
        let module = String::from_utf8_lossy(&record.module);
        self.text_box(module)
    }

    /// View record prefix.
    pub fn record_prefix<'a>(
        &'a self,
        record: &'a Record,
        row: widget::Row<'a, Message>,
    ) -> widget::Row<'a, Message> {
        row.push(self.badge(record))
            .push(
                self.text_container(record.clock_str())
                    .with_tooltip(record.timestamp_str()),
            )
            .push(self.module(record))
    }

    /// View widget.
    pub fn view(&self) -> Element<'_, Message, ::iced_core::Theme, ::iced_widget::Renderer> {
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
                                .push(
                                    icon::Icon::new(assets::plus()).size(icon_size(self.font_size)),
                                )
                                .pipe(|row| self.record_prefix(record, row))
                                .push(self.text_container(String::from_utf8_lossy(short)))
                                .push(self.text_box("..."))
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
                                        .push(
                                            icon::Icon::new(assets::minus())
                                                .size(icon_size(self.font_size)),
                                        )
                                        .pipe(|row| self.record_prefix(record, row)),
                                )
                                .push(self.text_box(String::from_utf8_lossy(&record.message)))
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
                            .pipe(|row| self.record_prefix(record, row))
                            .push(self.text_container(String::from_utf8_lossy(&record.message))),
                    )
                }
            })
            .spacing(3)
            .pipe(::spel_katalog_widget::xy_scrollable)
            .anchor_bottom()
            .width(Fill)
            .height(Fill)
            .pipe(widget::container)
            .style(widget::container::bordered_box)
            .padding(3)
            .height(Fill)
            .into()
    }
}
