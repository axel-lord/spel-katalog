//! Library for terminal widget.

use ::std::{
    borrow::Cow,
    collections::VecDeque,
    convert,
    fmt::Display,
    io::{ErrorKind, PipeReader, Read},
    mem,
    num::NonZero,
};

use ::iced::{Alignment::Center, Element, Length::Fill, Task, widget};
use ::spel_katalog_common::w;
use ::spel_katalog_sink::SinkIdentity;
use ::tokio_stream::wrappers::ReceiverStream;

/// Remove ansi escape codes from input.
fn without_ansi_escapes(bytes: Cow<'_, str>) -> Cow<'_, str> {
    #[derive(Debug)]
    struct Cleaner(String);

    impl ::vte::Perform for Cleaner {
        fn print(&mut self, c: char) {
            self.0.push(c);
        }
    }

    let Some((prior, bytes)) =
        ::memchr::memchr(b'\x1B', bytes.as_bytes()).and_then(|idx| bytes.split_at_checked(idx))
    else {
        return bytes;
    };

    let mut buf = String::from(prior);
    buf.reserve(bytes.len());

    let mut cleaner = Cleaner(buf);
    let mut parser = ::vte::Parser::new();

    parser.advance(&mut cleaner, bytes.as_bytes());

    let Cleaner(output) = cleaner;
    Cow::Owned(output)
}

/// Extend a string lossily from bytes.
fn extend_lossy(buf: &mut String, bytes: &[u8]) {
    for chunk in bytes.utf8_chunks() {
        buf.push_str(chunk.valid());
        let count = chunk.invalid().len();
        buf.reserve(count);
        for _ in 0..count {
            buf.push('\u{FFFD}');
        }
    }
}

/// Messages used by terminal.
#[derive(Debug)]
pub enum Message {
    /// Add a new pipe to terminal.
    AddPipe {
        /// Identity of pipe.
        identity: SinkIdentity,
        /// Pipe to add.
        reader: PipeReader,
    },
    /// Close a pipe.
    ClosePipe {
        #[doc(hidden)]
        idx: Private<usize>,
    },
    /// Add content.
    AddContent {
        #[doc(hidden)]
        idx: Private<usize>,
        #[doc(hidden)]
        content: Private<Vec<u8>>,
    },
    /// Set current viewed output.
    SetCurrent {
        /// Index of output.
        idx: Option<usize>,
    },
    /// Set wrapping used.
    SetWrap(Wrap),
    /// Attempt to set line count.
    SetLineCount(String),
    /// Set text size.
    SetTextSize(u16),
}

impl Message {
    /// Create a task receiving pipes.
    pub fn sink_receiver(
        recv: ::std::sync::mpsc::Receiver<(PipeReader, SinkIdentity)>,
    ) -> Task<Self> {
        Task::future(::tokio::task::spawn_blocking(move || {
            let value = recv.recv();

            (value, recv)
        }))
        .then(|result| {
            let ((reader, identity), recv) = match result {
                Err(err) => {
                    ::log::error!("sink receiver task could not be joined\n{err}");
                    return Task::none();
                }
                Ok((Err(_), _)) => {
                    return Task::none();
                }
                Ok((Ok(value), recv)) => (value, recv),
            };

            let next = Self::sink_receiver(recv);
            let value = Task::done(Message::AddPipe { identity, reader });
            Task::batch([value, next])
        })
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct Private<T>(pub(crate) T);

/// Storage for data received from pipe.
#[derive(Debug)]
struct Pipe {
    /// Id to display for this data.
    identity: String,
    /// All received content.
    content: Vec<u8>,
    /// If the pipe is still open.
    open: bool,
}

/// Id of data pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct PipeId<'s> {
    /// Name of pipe.
    name: &'s str,
    /// Index of pipe.
    idx: usize,
    /// If the pipe is still open.
    open: bool,
}

impl<'a> From<(usize, &'a Pipe)> for PipeId<'a> {
    fn from((idx, pipe): (usize, &'a Pipe)) -> Self {
        Self {
            name: &pipe.identity,
            idx,
            open: pipe.open,
        }
    }
}

impl Display for PipeId<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id = self.idx;
        let name = self.name;
        let status = if self.open { "" } else { " (closed)" };
        write!(f, "[{id}] {name}{status}")
    }
}

/// How lines wrapped in terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Wrap {
    /// Use no wrapping.
    #[default]
    None,
    /// Use word wrapping.
    Word,
    /// Use glyph wrapping.
    Glyph,
    /// Use word wrapping falling back on glyph wrapping.
    WordGlyph,
}

impl Display for Wrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Wrap::None => f.write_str("None"),
            Wrap::Word => f.write_str("Word"),
            Wrap::Glyph => f.write_str("Glyph"),
            Wrap::WordGlyph => f.write_str("Either"),
        }
    }
}

impl From<Wrap> for widget::text::Wrapping {
    fn from(value: Wrap) -> Self {
        match value {
            Wrap::None => Self::None,
            Wrap::Word => Self::Word,
            Wrap::Glyph => Self::Glyph,
            Wrap::WordGlyph => Self::WordOrGlyph,
        }
    }
}

/// Terminal widget/window.
#[derive(Debug)]
pub struct Terminal {
    /// Received data/pipes.
    pipes: Vec<Pipe>,
    /// Currently displayed lines.
    lines: VecDeque<(NonZero<usize>, String)>,
    /// Current pipe.
    current: Option<usize>,
    /// How to wrap content.
    wrap: Wrap,
    /// Default value for how many lines to display at most.
    limit: u16,
    /// Placeholder text for limit input.
    limit_placeholder: String,
    /// How many lines to currently display at most.
    current_limit: Option<u16>,
    /// Text in limit input.
    limit_text: String,
    /// Text size to use.
    text_size: u16,
}

impl Default for Terminal {
    fn default() -> Self {
        Self {
            pipes: Default::default(),
            lines: Default::default(),
            current: Default::default(),
            wrap: Default::default(),
            limit: Default::default(),
            limit_placeholder: Default::default(),
            current_limit: Default::default(),
            limit_text: Default::default(),
            text_size: 14,
        }
    }
}

impl Terminal {
    /// Get a terminal with limit set to given value.
    pub fn with_limit(self, limit: u16) -> Self {
        Self {
            limit,
            limit_placeholder: limit.to_string(),
            ..self
        }
    }

    /// Update state of terminal.
    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::AddPipe {
                identity,
                mut reader,
            } => {
                let (tx, rx) = ::tokio::sync::mpsc::channel(64);

                let pipe = Pipe {
                    identity: identity.to_string(),
                    content: Vec::new(),
                    open: true,
                };

                let idx = self.pipes.len();
                self.pipes.push(pipe);

                let close_task = Task::future(::tokio::task::spawn_blocking(move || {
                    let mut buf = vec![0; 1024];

                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break Ok(()),
                            Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                            Err(err) => break Err(err),
                            Ok(count) => {
                                if let Err(err) = tx.blocking_send(Vec::from(&buf[..count])) {
                                    break Err(::std::io::Error::other(err));
                                }
                            }
                        }
                    }
                }))
                .then(move |result| {
                    match result {
                        Err(err) => {
                            ::log::error!("could not run task for pipe {identity}\n{err}");
                        }
                        Ok(Err(err)) => {
                            ::log::error!("task reading pipe {identity} failed\n{err}");
                        }
                        Ok(..) => {}
                    }
                    Task::done(Message::ClosePipe { idx: Private(idx) })
                });

                let content_task =
                    Task::stream(ReceiverStream::new(rx)).map(move |content| Message::AddContent {
                        idx: Private(idx),
                        content: Private(content),
                    });

                let set_current = Task::done(Message::SetCurrent { idx: Some(idx) });

                Task::batch([close_task, content_task, set_current])
            }
            Message::ClosePipe { idx: Private(idx) } => {
                if let Some(pipe) = self.pipes.get_mut(idx) {
                    pipe.open = false;
                }
                Task::none()
            }
            Message::AddContent {
                idx: Private(idx),
                content: Private(new_content),
            } => {
                self.add_content(idx, new_content);
                Task::none()
            }
            Message::SetWrap(wrap) => {
                self.wrap = wrap;
                Task::none()
            }
            Message::SetCurrent { idx } => {
                if self.current != idx {
                    self.current = idx;
                    self.refresh();
                }
                Task::none()
            }
            Message::SetLineCount(count) => {
                self.set_line_count(count);
                Task::none()
            }
            Message::SetTextSize(size) => {
                self.text_size = size.clamp(7, 36);
                Task::none()
            }
        }
    }

    /// Add line to back of deque,
    fn add_line_back(lines: &mut VecDeque<(NonZero<usize>, String)>, line: Cow<str>) {
        if let Some((count, last)) = lines.back_mut()
            && last.as_str() == line
        {
            *count = count.saturating_add(1);
        } else {
            lines.push_back((const { NonZero::new(1).unwrap() }, line.into_owned()));
        }
    }

    /// Add line to front of deque.
    fn add_line_front(lines: &mut VecDeque<(NonZero<usize>, String)>, line: Cow<str>) {
        if let Some((count, first)) = lines.front_mut()
            && first.as_str() == line
        {
            *count = count.saturating_add(1);
        } else {
            lines.push_front((const { NonZero::new(1).unwrap() }, line.into_owned()));
        }
    }

    /// Refresh lines from byte content.
    fn refresh(&mut self) {
        let Self {
            pipes,
            lines,
            current,
            limit,
            current_limit,
            ..
        } = self;

        lines.clear();

        let Some(current) = *current else {
            return;
        };

        let Some(pipe) = pipes.get(current) else {
            return;
        };

        let mut content = pipe.content.as_slice();
        let limit = usize::from(current_limit.unwrap_or(*limit));

        for split_at in ::memchr::memrchr_iter(b'\n', &pipe.content) {
            let bytes = &content
                .split_off(split_at..)
                .expect("range should be in bounds")[1..];
            Self::add_line_front(lines, without_ansi_escapes(String::from_utf8_lossy(bytes)));

            if lines.len() >= limit {
                break;
            }
        }
    }

    /// Add more content to pipe with given index.
    fn add_content(&mut self, idx: usize, new_content: Vec<u8>) {
        let Some(pipe) = self.pipes.get_mut(idx) else {
            ::log::warn!("received content for unavailable task {idx}");
            return;
        };

        // name of content that we manipulate.
        let mut content = new_content.as_slice();
        let mut content = ::std::iter::from_fn(move || {
            if content.is_empty() {
                return None;
            }

            let Some(idx) = ::memchr::memchr(b'\n', content) else {
                let slice = content;
                content = &[];
                return Some(slice);
            };

            let slice = content.split_off(..idx).expect("idx should be in bounds");
            assert_eq!(content.split_off(..1), Some(b"\n".as_slice()));

            Some(slice)
        });

        if !self.lines.is_empty()
            && !pipe.content.ends_with(b"\n")
            && let Some(slice) = content.next()
        {
            let count = self.lines.back().map_or(1, |(count, _)| count.get());

            if count == 1 {
                let (_, last) = self.lines.back_mut().expect("length should be > 0");
                extend_lossy(last, slice);
                *last = without_ansi_escapes(mem::take(last).into()).into_owned();
            } else {
                let mut line = {
                    let (c, last) = self.lines.back_mut().expect("length should be > 0");
                    *c = NonZero::new(count - 1).expect("> 1 value - 1 should be > 0");
                    last.clone()
                };
                extend_lossy(&mut line, slice);
                self.lines
                    .push_back((const { NonZero::new(1).unwrap() }, line));
            }
        }
        pipe.content.extend_from_slice(&new_content);

        for slice in content {
            Self::add_line_back(
                &mut self.lines,
                without_ansi_escapes(String::from_utf8_lossy(slice)),
            );
        }

        while self.lines.len() > usize::from(self.limit) {
            self.lines.pop_front();
        }
    }

    /// Set amount of visible lines.
    fn set_line_count(&mut self, to: String) {
        if to.is_empty() {
            if self.current_limit.is_some() {
                self.current_limit = None;
                self.limit_text = String::new();
                self.refresh();
            }
            return;
        }

        let Ok(count) = to.parse() else {
            return;
        };

        if self.current_limit != Some(count) {
            self.current_limit = Some(count);
            self.limit_text = count.to_string();
            self.refresh();
        }
    }

    /// View terminal.
    pub fn view(&self) -> Element<'_, Message> {
        widget::Column::new()
            .padding(3)
            .spacing(3)
            .push(
                w::scroll(
                    widget::container(
                        self.lines
                            .iter()
                            .fold(widget::Column::new(), |column, line| {
                                column.push(
                                    widget::Text::new(line.1.as_str())
                                        .size(self.text_size)
                                        .font(::iced::font::Font::MONOSPACE)
                                        .wrapping(self.wrap.into()),
                                )
                            })
                            .spacing(3),
                    )
                    .style(widget::container::dark)
                    .width(Fill),
                )
                .anchor_bottom()
                .height(Fill),
            )
            .push(widget::horizontal_rule(3))
            .push(
                widget::Row::new()
                    .align_y(Center)
                    .spacing(3)
                    .push_maybe((!self.pipes.is_empty()).then(|| {
                        Element::from(
                            widget::pick_list(
                                self.pipes
                                    .iter()
                                    .enumerate()
                                    .map(PipeId::from)
                                    .collect::<Vec<_>>(),
                                self.current.and_then(|idx| {
                                    let pipe = self.pipes.get(idx)?;
                                    Some(PipeId {
                                        idx,
                                        name: &pipe.identity,
                                        open: pipe.open,
                                    })
                                }),
                                |PipeId { idx, .. }| Some(idx),
                            )
                            .padding(3),
                        )
                        .map(|idx| Message::SetCurrent { idx })
                    }))
                    .push(widget::horizontal_space())
                    .push("Size")
                    .push(
                        Element::from(
                            widget::pick_list(
                                [8, 9, 10, 11, 12, 14, 16, 18, 20],
                                Some(self.text_size),
                                convert::identity,
                            )
                            .padding(3),
                        )
                        .map(Message::SetTextSize),
                    )
                    .push("Wrapping:")
                    .push(
                        Element::from(
                            widget::pick_list(
                                [Wrap::None, Wrap::WordGlyph, Wrap::Word, Wrap::Glyph],
                                Some(self.wrap),
                                convert::identity,
                            )
                            .padding(3),
                        )
                        .map(Message::SetWrap),
                    )
                    .push("Lines:")
                    .push(
                        Element::from(
                            widget::text_input(&self.limit_placeholder, &self.limit_text)
                                .on_input(convert::identity)
                                .width(50)
                                .padding(3),
                        )
                        .map(Message::SetLineCount),
                    ),
            )
            .into()
    }
}
