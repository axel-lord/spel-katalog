//! TUI implementation.
use ::std::{
    borrow::Cow,
    io,
    num::NonZero,
    sync::mpsc::TryRecvError,
    thread::{self, JoinHandle},
    time::Duration,
};

use ::derive_more::Display;
use ::ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEvent},
    layout::{Constraint, Direction, Layout, Margin, Position as TermPos, Rect},
    style::Stylize,
    text::{Line, Text},
    widgets::{Block, Clear, Paragraph},
};
use ::tap::Pipe as _;

use crate::{Channels, LineReceiver, SinkIdentity, line_channel};

#[derive(Debug)]
struct Pipe {
    line_rx: LineReceiver,
    handle: Option<JoinHandle<io::Result<u64>>>,
    id: SinkIdentity,
    is_disconnected: bool,
    scroll: ScrollPos,
}

#[derive(Debug, Clone, Copy)]
enum SelectedArea {
    Log,
    Pipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Action {
    Exit,
    SelectOutput,
    SelectLog,
    Resize,
    Noop,
    BtnLeft(TermPos),
    ScrollUpAt(TermPos),
    ScrollDownAt(TermPos),
    ScrollUp,
    ScrollDown,
    PgUp,
    PgDn,
    NextOutput,
    PrevOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Display)]
enum ScrollPos {
    Start(usize),
    #[default]
    End,
}

impl ScrollPos {
    fn by(self, value: isize, area: Rect, len: usize) -> Self {
        let height = area.height as usize;

        let pos = match self {
            ScrollPos::Start(idx) => idx,
            ScrollPos::End => len.saturating_sub(height),
        };
        let pos = pos.saturating_add_signed(value);

        if pos + height >= len {
            Self::End
        } else {
            Self::Start(pos)
        }
    }

    /// Scroll up.
    fn up(self, area: Rect, len: usize) -> Self {
        self.by(-1, area, len)
    }

    /// Scroll down.
    fn down(self, area: Rect, len: usize) -> Self {
        self.by(1, area, len)
    }

    /// Scroll up one page.
    fn pg_up(self, area: Rect, len: usize) -> Self {
        self.by(-(area.height as isize), area, len)
    }

    /// Scroll down one page.
    fn pg_down(self, area: Rect, len: usize) -> Self {
        self.by(area.height as isize, area, len)
    }
}

fn line_rx_paragraph(line_rx: &LineReceiver, area: Rect, scroll: ScrollPos) -> Paragraph<'_> {
    fn to_line<'a>((count, line): &'a (NonZero<usize>, String)) -> Cow<'a, str> {
        if count.get() > 1 {
            Cow::Owned(format!("[x{count}] {line}"))
        } else {
            Cow::Borrowed(line.as_str())
        }
    }

    let height = area.height as usize;

    let lines = match scroll {
        ScrollPos::End => line_rx.as_slice(),
        ScrollPos::Start(off) => line_rx.get(..(off + height)).unwrap_or(line_rx.as_slice()),
    };

    lines
        .rchunks(height)
        .next()
        .unwrap_or(&[])
        .iter()
        .map(to_line)
        .pipe(Text::from_iter)
        .pipe(Paragraph::new)
}

pub fn tui(channels: Channels, terminal: &mut DefaultTerminal) -> io::Result<()> {
    let Channels {
        exit_tx,
        pipe_rx,
        mut log_rx,
    } = channels;

    let mut pipes = Vec::new();
    let mut latest = None;
    let mut selected_area = SelectedArea::Log;
    let mut log_area = Rect::ZERO;
    let mut output_area = Rect::ZERO;
    let mut log_scroll = ScrollPos::End;

    fn with_latest(latest: Option<usize>, pipes: &mut [Pipe], apply: impl FnOnce(&mut Pipe)) {
        if let Some(pipe) = latest.and_then(|idx| pipes.get_mut(idx)) {
            apply(pipe)
        }
    }

    loop {
        terminal.draw(|frame| {
            let draw_log = |frame: &mut Frame, log_area: Rect| {
                let block = if matches!(selected_area, SelectedArea::Log) {
                    Block::bordered().title_bottom("Application Log".bold().blue())
                } else {
                    Block::bordered().title_bottom("Application Log")
                }
                .title_bottom(Line::from(format!("{log_scroll}/{}", log_rx.len())).right_aligned());
                let area = block.inner(log_area);
                frame.render_widget(block, log_area);

                let lines =
                    Paragraph::new(Text::from_iter(log_rx.iter().map(|item| item.1.as_str())));
                frame.render_widget(lines, area);

                let lines = line_rx_paragraph(&log_rx, area, log_scroll);

                frame.render_widget(Clear, area);
                frame.render_widget(lines, area);
            };

            if let Some((
                Pipe {
                    id,
                    line_rx,
                    is_disconnected: closed,
                    scroll,
                    ..
                },
                idx,
            )) = latest.and_then(|idx: usize| Some((pipes.get_mut(idx)?, idx)))
            {
                let layout = Layout::new(
                    Direction::Vertical,
                    [Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)],
                )
                .areas::<2>(frame.area());
                [output_area, log_area] = layout;

                draw_log(frame, log_area);

                let title = if *closed {
                    format!("[{idx}] {id} (closed)")
                } else {
                    format!("[{idx}] {id}")
                };
                let block = if matches!(selected_area, SelectedArea::Pipe) {
                    Block::bordered().title_bottom(title.bold().blue())
                } else {
                    Block::bordered().title_bottom(title)
                }
                .title_bottom(Line::from(format!("{scroll}/{}", line_rx.len())).right_aligned());
                let area = block.inner(output_area);

                frame.render_widget(block, output_area);

                let lines = line_rx_paragraph(line_rx, area, *scroll);

                frame.render_widget(Clear, area);
                frame.render_widget(lines, area);
            } else {
                let area = frame.area();
                log_area = area;
                draw_log(frame, area);
            }
        })?;

        loop {
            let mut should_redraw = false;
            fn scroll_fn(
                f: impl FnOnce(ScrollPos, Rect, usize) -> ScrollPos,
                line_rx: &LineReceiver,
                scroll: &mut ScrollPos,
                area: Rect,
            ) {
                *scroll = f(*scroll, area.inner(Margin::new(1, 1)), line_rx.len());
            }
            if event::poll(Duration::from_secs_f64(1.0 / 10.0))? {
                match handle_events()? {
                    Action::Exit => {
                        exit_tx();
                        return Ok(());
                    }
                    Action::Noop | Action::Resize => {}
                    Action::BtnLeft(term_pos) => {
                        if log_area.contains(term_pos) {
                            selected_area = SelectedArea::Log;
                        } else if output_area.contains(term_pos) {
                            selected_area = SelectedArea::Pipe;
                        }
                    }
                    Action::ScrollUpAt(term_pos) => {
                        if log_area.contains(term_pos) {
                            selected_area = SelectedArea::Log;
                            log_scroll =
                                log_scroll.up(log_area.inner(Margin::new(1, 1)), log_rx.len());
                        } else if output_area.contains(term_pos) {
                            selected_area = SelectedArea::Pipe;
                            with_latest(latest, &mut pipes, |latest| {
                                scroll_fn(
                                    ScrollPos::up,
                                    &latest.line_rx,
                                    &mut latest.scroll,
                                    output_area,
                                );
                            });
                        }
                    }
                    Action::ScrollDownAt(term_pos) => {
                        if log_area.contains(term_pos) {
                            selected_area = SelectedArea::Log;
                            scroll_fn(ScrollPos::down, &log_rx, &mut log_scroll, log_area);
                        } else if output_area.contains(term_pos) {
                            selected_area = SelectedArea::Pipe;
                            with_latest(latest, &mut pipes, |latest| {
                                scroll_fn(
                                    ScrollPos::down,
                                    &latest.line_rx,
                                    &mut latest.scroll,
                                    output_area,
                                );
                            });
                        }
                    }
                    Action::PgUp => match selected_area {
                        SelectedArea::Log => {
                            scroll_fn(ScrollPos::pg_up, &log_rx, &mut log_scroll, log_area);
                        }
                        SelectedArea::Pipe => with_latest(latest, &mut pipes, |latest| {
                            scroll_fn(
                                ScrollPos::pg_up,
                                &latest.line_rx,
                                &mut latest.scroll,
                                output_area,
                            )
                        }),
                    },
                    Action::PgDn => match selected_area {
                        SelectedArea::Log => {
                            scroll_fn(ScrollPos::pg_down, &log_rx, &mut log_scroll, log_area)
                        }
                        SelectedArea::Pipe => with_latest(latest, &mut pipes, |latest| {
                            scroll_fn(
                                ScrollPos::pg_down,
                                &latest.line_rx,
                                &mut latest.scroll,
                                output_area,
                            )
                        }),
                    },
                    Action::ScrollUp => match selected_area {
                        SelectedArea::Log => {
                            scroll_fn(ScrollPos::up, &log_rx, &mut log_scroll, log_area)
                        }
                        SelectedArea::Pipe => with_latest(latest, &mut pipes, |latest| {
                            scroll_fn(
                                ScrollPos::up,
                                &latest.line_rx,
                                &mut latest.scroll,
                                output_area,
                            )
                        }),
                    },
                    Action::ScrollDown => match selected_area {
                        SelectedArea::Log => {
                            scroll_fn(ScrollPos::down, &log_rx, &mut log_scroll, log_area)
                        }
                        SelectedArea::Pipe => with_latest(latest, &mut pipes, |latest| {
                            scroll_fn(
                                ScrollPos::down,
                                &latest.line_rx,
                                &mut latest.scroll,
                                output_area,
                            )
                        }),
                    },
                    Action::NextOutput => {
                        if let Some(idx) = latest {
                            selected_area = SelectedArea::Pipe;
                            latest = Some((pipes.len() - 1).min(idx + 1));
                        }
                    }
                    Action::PrevOutput => {
                        if let Some(idx) = latest {
                            selected_area = SelectedArea::Pipe;
                            latest = Some(idx.saturating_sub(1));
                        }
                    }
                    Action::SelectOutput => {
                        if latest.is_some() {
                            selected_area = SelectedArea::Pipe;
                        }
                    }
                    Action::SelectLog => selected_area = SelectedArea::Log,
                }
                should_redraw = true;
            };

            match pipe_rx.try_recv() {
                Ok((mut r, id)) => {
                    let (mut line_pipe, line_rx) = line_channel();
                    let handle = Some(thread::spawn(move || io::copy(&mut r, &mut line_pipe)));

                    pipes.push(Pipe {
                        line_rx,
                        handle,
                        id,
                        is_disconnected: false,
                        scroll: ScrollPos::End,
                    });
                }
                Err(err) => {
                    if let TryRecvError::Disconnected = err {
                        exit_tx();
                        return Ok(());
                    }
                }
            }

            for (
                idx,
                Pipe {
                    line_rx,
                    is_disconnected,
                    handle,
                    id,
                    ..
                },
            ) in pipes.iter_mut().enumerate()
            {
                if *is_disconnected {
                    continue;
                }

                match line_rx.try_recv_many(128) {
                    // Redraw if any lines where received.
                    Ok(_count) => {
                        should_redraw = true;
                        latest = Some(idx);
                    }

                    // Join handle if disconnected.
                    Err(TryRecvError::Disconnected) => {
                        *is_disconnected = true;
                        if let Some(handle) = handle.take() {
                            match handle.join() {
                                Ok(result) => match result {
                                    Ok(count) => ::log::info!(
                                        "pipe for {id} closed with {count} bytes written"
                                    ),
                                    Err(err) => ::log::error!(
                                        "error copying bytes from pipe for {id}\n{err}"
                                    ),
                                },
                                Err(err) => ::std::panic::resume_unwind(err),
                            }
                        }
                    }

                    // Do nothing if no lines received.
                    Err(TryRecvError::Empty) => {}
                }
            }

            match log_rx.try_recv_many(128) {
                // Redraw if any log lines where received.
                Ok(_c) => {
                    should_redraw = true;
                }

                // Exit if log disconnected.
                Err(TryRecvError::Disconnected) => {
                    exit_tx();
                    return Ok(());
                }

                // Continue on if nothing was received.
                Err(TryRecvError::Empty) => {}
            }

            if should_redraw {
                break;
            }
        }
    }
}

fn handle_events() -> io::Result<Action> {
    Ok(match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Char('q') => Action::Exit,

            KeyCode::Up if key.modifiers.intersects(KeyModifiers::ALT) => Action::SelectOutput,
            KeyCode::Down if key.modifiers.intersects(KeyModifiers::ALT) => Action::SelectLog,

            KeyCode::Left => Action::PrevOutput,
            KeyCode::Right => Action::NextOutput,

            KeyCode::Up => Action::ScrollUp,
            KeyCode::Down => Action::ScrollDown,

            KeyCode::PageUp => Action::PgUp,
            KeyCode::PageDown => Action::PgDn,
            _ => return Ok(Action::Noop),
        },
        Event::Mouse(MouseEvent {
            column: x,
            row: y,
            kind,
            ..
        }) => match kind {
            event::MouseEventKind::Down(event::MouseButton::Left) => {
                Action::BtnLeft(TermPos { x, y })
            }
            event::MouseEventKind::ScrollUp => Action::ScrollUpAt(TermPos { x, y }),
            event::MouseEventKind::ScrollDown => Action::ScrollDownAt(TermPos { x, y }),
            _ => return Ok(Action::Noop),
        },
        Event::Resize(..) => Action::Resize,
        _ => return Ok(Action::Noop),
    })
}
