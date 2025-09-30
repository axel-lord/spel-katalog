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
    layout::Margin,
    text::Text,
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
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
    handle: JoinHandle<io::Result<u64>>,
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

    let mut count = 0usize;
    let mut log = Vec::new();
    let mut pipes = Vec::new();
    let mut latest = None;

    loop {
        count += 1;
        terminal.draw(|frame| {
            let (output_id, output): (_, &[String]) =
                if let Some(Pipe { id, lines, .. }) = latest.and_then(|latest| pipes.get(latest)) {
                    (Some(id), lines.as_slice())
                } else {
                    (None, Default::default())
                };

            {
                // let text = Paragraph::new(format!("Placeholder! Iteration {count}"));
                // frame.render_widget(text, frame.area());

                let draw_log = |frame: &mut Frame| {
                    let area = frame.area();
                    let mut state = ScrollbarState::new(log.len()).position(log.len());
                    let log = Paragraph::new(Text::from_iter(
                        log.iter().map(|item: &String| item.as_str()),
                    ));
                    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
                    frame.render_widget(log, area);
                    frame.render_stateful_widget(
                        scrollbar,
                        area.inner(Margin {
                            vertical: 1,
                            horizontal: 0,
                        }),
                        &mut state,
                    );
                };

                if let Some(id) = output_id {
                    _ = id;
                    draw_log(frame)
                } else {
                    draw_log(frame)
                }
            }
        })?;

        loop {
            let mut should_break = false;
            if event::poll(Duration::from_secs_f64(1.0 / 10.0))? {
                if handle_events()? {
                    exit_tx();
                    return Ok(());
                }
                should_break = true;
            };

            match pipe_rx.try_recv() {
                Ok((mut r, id)) => {
                    let (mut line_pipe, line_rx) = LinePipe::channel();
                    let handle = thread::spawn(move || io::copy(&mut r, &mut line_pipe));

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
                    ..
                },
            ) in pipes.iter_mut().enumerate()
            {
                if *is_disconnected {
                    continue;
                }
                match line_rx.try_recv() {
                    Ok(line) => {
                        lines.push(bytes_to_string(line));
                        latest = Some(idx);
                    }
                    Err(err) => {
                        if let TryRecvError::Disconnected = err {
                            *is_disconnected = true
                        }
                    }
                }
            }

            match log_rx.try_recv() {
                Ok(line) => {
                    log.push(bytes_to_string(line));
                    should_break = true;
                }
                Err(err) => {
                    if let TryRecvError::Disconnected = err {
                        exit_tx();
                        return Ok(());
                    }
                }
            }

            if should_break {
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
