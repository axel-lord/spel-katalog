//! TUI implementation.
use ::std::{
    borrow::Cow,
    io,
    sync::mpsc::TryRecvError,
    thread::{self, JoinHandle},
    time::Duration,
};

use ::ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    text::Text,
    widgets::{Block, Paragraph},
};
use ::tap::Pipe as _;

use crate::{Channels, LineReceiver, SinkIdentity, line_channel};

#[derive(Debug)]
struct Pipe {
    line_rx: LineReceiver,
    handle: Option<JoinHandle<io::Result<u64>>>,
    id: SinkIdentity,
    is_disconnected: bool,
}

#[derive(Debug, Clone, Copy)]
enum SelectedArea {
    Log,
    Pipe,
}

#[derive(Debug, Clone, Copy)]
enum Action {
    Exit,
    SwapSelected,
    Noop,
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

    loop {
        terminal.draw(|frame| {
            let draw_log = |frame: &mut Frame, area: Rect| {
                let lines =
                    Paragraph::new(Text::from_iter(log_rx.iter().map(|item| item.1.as_str())));
                frame.render_widget(lines, area);

                let lines = log_rx
                    .rchunks(area.height as usize)
                    .next()
                    .unwrap_or(&[])
                    .iter()
                    .map(|(count, line)| {
                        if count.get() > 1 {
                            Cow::Owned(format!("[x{count}] {line}"))
                        } else {
                            Cow::Borrowed(line.as_str())
                        }
                    })
                    .pipe(Text::from_iter)
                    .pipe(Paragraph::new);

                frame.render_widget(lines, area);
            };

            if let Some((
                Pipe {
                    id,
                    line_rx,
                    is_disconnected: closed,
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

                let block = if matches!(selected_area, SelectedArea::Log) {
                    Block::bordered().title_bottom("Application Log".bold().blue())
                } else {
                    Block::bordered().title_bottom("Application Log")
                };
                let area = block.inner(layout[1]);
                frame.render_widget(block, layout[1]);

                draw_log(frame, area);

                let title = if *closed {
                    format!("[{idx}] {id} (closed)")
                } else {
                    format!("[{idx}] {id}")
                };
                let block = if matches!(selected_area, SelectedArea::Pipe) {
                    Block::bordered().title_bottom(title.bold().blue())
                } else {
                    Block::bordered().title_bottom(title)
                };
                let area = block.inner(layout[0]);

                frame.render_widget(block, layout[0]);

                let lines = line_rx
                    .rchunks(area.height as usize)
                    .next()
                    .unwrap_or(&[])
                    .iter()
                    .map(|(count, line)| {
                        if count.get() > 1 {
                            Cow::Owned(format!("[x{count}] {line}"))
                        } else {
                            Cow::Borrowed(line.as_str())
                        }
                    })
                    .pipe(Text::from_iter)
                    .pipe(Paragraph::new);

                frame.render_widget(lines, area);
            } else {
                let area = frame.area();
                draw_log(frame, area);
            }
        })?;

        loop {
            let mut should_redraw = false;
            if event::poll(Duration::from_secs_f64(1.0 / 10.0))? {
                match handle_events()? {
                    Action::Exit => {
                        exit_tx();
                        return Ok(());
                    }
                    Action::SwapSelected => match selected_area {
                        SelectedArea::Log => {
                            if latest.is_some() {
                                selected_area = SelectedArea::Pipe;
                            }
                        }
                        SelectedArea::Pipe => selected_area = SelectedArea::Log,
                    },
                    Action::Noop => {}
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
    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Char('q') => return Ok(Action::Exit),
            KeyCode::Left | KeyCode::Right => return Ok(Action::SwapSelected),
            _ => {}
        },
        _ => {}
    }
    Ok(Action::Noop)
}
