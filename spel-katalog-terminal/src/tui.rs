//! TUI implementation.
use ::std::{
    io,
    sync::mpsc::{Receiver, TryRecvError},
    thread::{self, JoinHandle},
    time::Duration,
};

use ::ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Direction, Layout, Margin, Rect},
    text::Text,
    widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

use crate::{Channels, LinePipe, SinkIdentity};

fn bytes_to_string(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(err) => {
            use ::std::fmt::Write;
            let bytes = err.as_bytes();
            let mut buf = String::with_capacity(bytes.len());
            for chunk in err.as_bytes().utf8_chunks() {
                buf.push_str(chunk.valid());
                for byte in chunk.invalid() {
                    write!(buf, "\\x{:02X}", byte).expect("write to String should succeed");
                }
            }
            buf
        }
    }
}

#[derive(Debug)]
struct Pipe {
    line_rx: Receiver<Vec<u8>>,
    handle: Option<JoinHandle<io::Result<u64>>>,
    id: SinkIdentity,
    is_disconnected: bool,
    lines: Vec<String>,
}

pub fn tui(channels: Channels, terminal: &mut DefaultTerminal) -> io::Result<()> {
    let Channels {
        exit_tx,
        pipe_rx,
        log_rx,
    } = channels;

    let mut log = Vec::new();
    let mut pipes = Vec::new();
    let mut latest = None;

    loop {
        terminal.draw(|frame| {
            let (output_id, output, closed): (_, &[String], _) = if let Some(Pipe {
                id,
                lines,
                is_disconnected,
                ..
            }) =
                latest.and_then(|latest| pipes.get(latest))
            {
                (Some(id), lines.as_slice(), *is_disconnected)
            } else {
                (None, Default::default(), true)
            };

            {
                // let text = Paragraph::new(format!("Placeholder! Iteration {count}"));
                // frame.render_widget(text, frame.area());

                let draw_log = |frame: &mut Frame, area: Rect| {
                    let lines = Paragraph::new(Text::from_iter(
                        log.iter().map(|item: &String| item.as_str()),
                    ));
                    frame.render_widget(lines, area);

                    if (area.height as usize) < log.len() {
                        let mut state = ScrollbarState::new(log.len()).position(log.len());
                        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                        frame.render_stateful_widget(
                            scrollbar,
                            area.inner(Margin {
                                vertical: 1,
                                horizontal: 0,
                            }),
                            &mut state,
                        );
                    }
                };

                if let Some(id) = output_id {
                    let layout = Layout::new(
                        Direction::Vertical,
                        [Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)],
                    )
                    .areas::<2>(frame.area());

                    let block = Block::bordered().title_bottom("Application Log");
                    let area = block.inner(layout[1]);
                    frame.render_widget(block, layout[1]);

                    draw_log(frame, area);

                    let lines =
                        Paragraph::new(Text::from_iter(output.iter().map(|item| item.as_str())));
                    let block = Block::bordered().title_bottom(if closed {
                        format!("{id} (closed)")
                    } else {
                        id.to_string()
                    });
                    let area = block.inner(layout[0]);

                    frame.render_widget(block, layout[0]);
                    frame.render_widget(lines, area);

                    if (area.height as usize) < output.len() {
                        let mut state = ScrollbarState::new(output.len()).position(output.len());
                        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                        frame.render_stateful_widget(
                            scrollbar,
                            area.inner(Margin {
                                horizontal: 0,
                                vertical: 1,
                            }),
                            &mut state,
                        );
                    }
                } else {
                    let area = frame.area();
                    draw_log(frame, area);
                }
            }
        })?;

        loop {
            let mut should_redraw = false;
            if event::poll(Duration::from_secs_f64(1.0 / 10.0))? {
                if handle_events()? {
                    exit_tx();
                    return Ok(());
                }
                should_redraw = true;
            };

            match pipe_rx.try_recv() {
                Ok((mut r, id)) => {
                    let (mut line_pipe, line_rx) = LinePipe::channel();
                    let handle = Some(thread::spawn(move || io::copy(&mut r, &mut line_pipe)));

                    pipes.push(Pipe {
                        line_rx,
                        handle,
                        id,
                        is_disconnected: false,
                        lines: Vec::new(),
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
                    lines,
                    handle,
                    id,
                    ..
                },
            ) in pipes.iter_mut().enumerate()
            {
                if *is_disconnected {
                    continue;
                }
                let mut receiving = true;
                while receiving {
                    match line_rx.try_recv() {
                        Ok(line) => {
                            lines.push(bytes_to_string(line));
                            latest = Some(idx);
                            should_redraw = true;
                        }
                        Err(err) => {
                            receiving = false;
                            if let TryRecvError::Disconnected = err {
                                *is_disconnected = true;
                                should_redraw = true;
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
                        }
                    }
                }
            }

            let mut receiving = true;
            while receiving {
                match log_rx.try_recv() {
                    Ok(line) => {
                        log.push(bytes_to_string(line));
                        should_redraw = true;
                    }
                    Err(err) => {
                        if let TryRecvError::Disconnected = err {
                            exit_tx();
                            return Ok(());
                        }
                        receiving = false;
                    }
                }
            }

            if should_redraw {
                break;
            }
        }
    }
}

fn handle_events() -> io::Result<bool> {
    match event::read()? {
        Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Char('q') => return Ok(true),
            _ => {}
        },
        _ => {}
    }
    Ok(false)
}
