//! TUI implementation.
use ::std::{
    io,
    sync::mpsc::TryRecvError,
    thread::{self, JoinHandle},
    time::Duration,
};

use ::ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::Stylize,
    text::Text,
    widgets::{Block, Paragraph, ScrollDirection, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::{Channels, LineReceiver, SinkIdentity, line_channel};

#[derive(Debug)]
struct Pipe {
    line_rx: LineReceiver,
    handle: Option<JoinHandle<io::Result<u64>>>,
    id: SinkIdentity,
    is_disconnected: bool,
    scroll_state: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
enum SelectedArea {
    Log,
    Pipe,
}

#[derive(Debug, Clone, Copy)]
enum Action {
    Exit,
    #[expect(unused)]
    Scroll(ScrollDirection),
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
    let mut log_scroll_state: Option<ScrollbarState> = None;
    let mut selected_area = SelectedArea::Log;

    loop {
        terminal.draw(|frame| {
            let mut draw_log = |frame: &mut Frame, area: Rect| {
                let lines =
                    Paragraph::new(Text::from_iter(log_rx.iter().map(|item| item.1.as_str())));
                frame.render_widget(lines, area);

                if (area.height as usize) < log_rx.len() {
                    let mut state;
                    let state = if let Some(state) = &mut log_scroll_state {
                        state
                    } else {
                        state = ScrollbarState::new(log_rx.len()).position(log_rx.len());
                        &mut state
                    };
                    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                    frame.render_stateful_widget(
                        scrollbar,
                        area.inner(Margin {
                            vertical: 1,
                            horizontal: 0,
                        }),
                        state,
                    );
                }
            };

            if let Some((
                Pipe {
                    id,
                    line_rx,
                    is_disconnected: closed,
                    scroll_state,
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

                let lines =
                    Paragraph::new(Text::from_iter(line_rx.iter().map(|item| item.1.as_str())));
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

                if (area.height as usize) < line_rx.len() {
                    let mut state;
                    let state = if let Some(pos) = scroll_state {
                        state = ScrollbarState::new(line_rx.len()).position(*pos);
                        &mut state
                    } else {
                        state = ScrollbarState::new(line_rx.len()).position(line_rx.len());
                        &mut state
                    };

                    frame.render_widget(
                        lines.scroll((scroll_state.unwrap_or(line_rx.len()) as u16, 0)),
                        area,
                    );
                    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                    frame.render_stateful_widget(
                        scrollbar,
                        area.inner(Margin {
                            horizontal: 0,
                            vertical: 1,
                        }),
                        state,
                    );
                } else {
                    frame.render_widget(lines, area);
                }
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
                    Action::Scroll(dir) => match selected_area {
                        SelectedArea::Log => {
                            log_scroll_state
                                .get_or_insert_with(|| {
                                    ScrollbarState::new(log_rx.len()).position(log_rx.len())
                                })
                                .scroll(dir);
                        }
                        SelectedArea::Pipe => {
                            if let Some(Pipe {
                                line_rx,
                                scroll_state,
                                ..
                            }) = latest.and_then(|idx: usize| pipes.get_mut(idx))
                            {
                                let pos = scroll_state.get_or_insert_with(|| line_rx.len());
                                match dir {
                                    ScrollDirection::Forward => {
                                        *pos += 1;
                                    }
                                    ScrollDirection::Backward => {
                                        *pos = pos.saturating_sub(1);
                                    }
                                }
                                if *pos >= line_rx.len() {
                                    *scroll_state = None;
                                }
                            }
                        }
                    },
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
                        scroll_state: None,
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
            /* KeyCode::Up => return Ok(Action::Scroll(ScrollDirection::Backward)),
            KeyCode::Down => return Ok(Action::Scroll(ScrollDirection::Forward)), */
            KeyCode::Left | KeyCode::Right => return Ok(Action::SwapSelected),
            _ => {}
        },
        _ => {}
    }
    Ok(Action::Noop)
}
