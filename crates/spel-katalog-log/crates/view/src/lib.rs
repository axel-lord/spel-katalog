//! View of log.

use ::iced_core::{
    Alignment::Center, Border, Element, Font, Length::Fill, Theme, text::IntoFragment,
};
use ::iced_runtime::{Task, futures::Subscription};
use ::iced_widget::{self as widget, Column};
use ::smol::stream::StreamExt;
use ::spel_katalog_assets as assets;
use ::spel_katalog_log::RecordMessage;
use ::spel_katalog_widget::{WidgetExt, icon};
use ::tap::Pipe;

use crate::{record::Record, record_conent::RecordContent};

mod record;
mod record_conent;

/// Message used by [LogView].
#[derive(Debug, Clone)]
pub enum Message {
    /// Receive a log record.
    RecvRecord(RecordMessage),
    /// Toggle log view.
    Toggle(usize),
    /// Set regex filter.
    Filter(String),
}

/// Log view.
#[derive(Debug)]
pub struct LogView {
    /// Stored records.
    records: Vec<RecordContent>,
    /// Font size of view.
    font_size: u32,
    /// String value of filter.
    filter: String,
    /// Current regex in use.
    regex: Option<::regex::bytes::Regex>,
}

impl Default for LogView {
    fn default() -> Self {
        Self {
            records: Default::default(),
            font_size: 13,
            filter: String::new(),
            regex: None,
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
            Message::Filter(matcher) => {
                if matcher.is_empty() {
                    self.regex = None;
                } else if let Ok(re) = ::regex::bytes::RegexBuilder::new(&matcher)
                    .case_insensitive(true)
                    .multi_line(true)
                    .build()
                {
                    self.regex = Some(re);
                }
                self.filter = matcher;

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
                    .with_text_tooltip(record.timestamp_str()),
            )
            .push(self.module(record))
    }

    /// Add record to column.
    fn add_record<'a>(
        &'a self,
        col: widget::Column<'a, Message>,
        record: &'a RecordContent,
        idx: usize,
    ) -> widget::Column<'a, Message> {
        if record.is_shortened() {
            match record {
                RecordContent::Closed { record, short } => col.push(
                    widget::Row::new()
                        .align_y(Center)
                        .spacing(3)
                        .push(icon::Icon::new(assets::plus()).size(icon_size(self.font_size)))
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
    }

    /// View records column.
    fn view_records(&self) -> Column<'_, Message> {
        if let Some(re) = &self.regex {
            self.records
                .iter()
                .filter(|record| re.is_match(&record.message))
                .enumerate()
                .fold(widget::Column::new(), |col, (idx, record)| {
                    self.add_record(col, record, idx)
                })
        } else {
            self.records
                .iter()
                .enumerate()
                .fold(widget::Column::new(), |col, (idx, record)| {
                    self.add_record(col, record, idx)
                })
        }
        .spacing(3)
    }

    /// View widget.
    pub fn view(&self) -> Element<'_, Message, ::iced_core::Theme, ::iced_widget::Renderer> {
        Column::new()
            .spacing(3)
            .push(
                widget::text_input("filter...", &self.filter)
                    .on_input(Message::Filter)
                    .padding(3),
            )
            .push(
                self.view_records()
                    .pipe(::spel_katalog_widget::xy_scrollable)
                    .anchor_bottom()
                    .width(Fill)
                    .height(Fill),
            )
            .pipe(widget::container)
            .style(widget::container::bordered_box)
            .padding(3)
            .height(Fill)
            .into()
    }
}
