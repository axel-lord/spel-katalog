//! Sample game, should run on all platforms, and log with color.

use ::core::time::Duration;
use ::std::io::Write;

use ::clap::Parser;
use ::iced::{
    Element,
    Length::Fill,
    Task, Theme,
    widget::{self, Button, Column, Row, button, text},
};
use ::log::LevelFilter;
use ::rand::{Rng, seq::SliceRandom};
use ::tap::Tap;

/// Cli
#[derive(Debug, Parser)]
#[command(author, version, long_about = None)]
struct Cli {}

/// Game message.
#[derive(Debug, Clone, Copy)]
enum Msg {
    /// Pressed tile.
    Pressed {
        /// X pos of tile.
        x: usize,
        /// Y pos of tile.
        y: usize,
    },
    /// Log a wave.
    Wave(f32),
    /// Fill tiles.
    Fill,
    /// Reset board.
    Reset,
}

/// A game cell.
#[derive(Debug, Clone)]
struct Cell {
    /// Is this cell selected.
    is_selected: bool,
    /// Value of cell.
    value: String,
}

impl Cell {
    /// View cell.
    pub fn view<'a>(&'a self, x: usize, y: usize, is_success: bool) -> Button<'a, Msg> {
        button(
            if self.is_selected {
                text(format!("[{}]", self.value))
            } else {
                text(&self.value)
            }
            .center()
            .height(Fill)
            .width(Fill),
        )
        .style(move |theme, status| {
            if self.is_selected && is_success {
                widget::button::success(theme, status)
            } else if self.is_selected {
                widget::button::primary(theme, status)
            } else {
                widget::button::secondary(theme, status)
            }
        })
        .on_press_with(move || Msg::Pressed { x, y })
    }
}

/// Game state.
#[derive(Debug, Default)]
struct State {
    /// Cells of board.
    cells: Vec<Vec<Cell>>,
    /// Dimension of board.
    dim: usize,
    /// Current duplicated tile.
    dupe: String,
    /// Is selection success.
    is_success: bool,
}

impl State {
    /// Reset state.
    fn reset(&mut self) {
        self.is_success = false;
        let range = (1, self.dim * self.dim);
        let mut rng = ::rand::rng();
        let dupe = rng.sample(::rand::distr::Uniform::new(range.0, range.1).unwrap());
        self.dupe = dupe.to_string();

        self.cells = Vec::from_iter(range.0..range.1)
            .tap_mut(|values| {
                values.push(dupe);
                values.shuffle(&mut rng);
            })
            .chunks(self.dim)
            .map(|chunk| {
                chunk
                    .iter()
                    .map(|n| Cell {
                        value: n.to_string(),
                        is_selected: false,
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        ::log::info!("reset board, dupe {}", self.dupe);
    }

    /// Get selected tiles.
    fn get_selected(&self) -> Vec<&str> {
        self.cells
            .iter()
            .flatten()
            .filter(|cell| cell.is_selected)
            .map(|cell| cell.value.as_str())
            .collect()
    }

    /// Update game state.
    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Pressed { x, y } => {
                let cell = &mut self.cells[y][x];
                cell.is_selected = !cell.is_selected;

                ::log::info!("tile ({x}, {y}) pressed");

                let selected = self.get_selected();

                self.is_success = selected.as_slice() == [self.dupe.as_str(), self.dupe.as_str()];

                if self.is_success {
                    ::log::info!("success! {}", self.dupe);
                }

                Task::none()
            }
            Msg::Fill => {
                self.dim += 1;
                self.dim = self.dim.max(2);
                ::log::info!("dim raised to {}", self.dim);
                self.reset();

                Task::none()
            }
            Msg::Reset => {
                self.dim = self.dim.max(2);
                self.reset();
                Task::none()
            }
            Msg::Wave(off) => {
                if off >= 128.0 {
                    Task::none()
                } else {
                    let mag = ((::core::f32::consts::PI * 2.0 * off / 64.0).sin() + 1.0) * 128.0;
                    let mut stdout = ::std::io::stdout().lock();

                    (0..(mag.round() as usize)).for_each(|_| {
                        stdout
                            .write_all(b"#")
                            .expect("write to stdout should succeed")
                    });
                    stdout
                        .write_all(b"\n")
                        .expect("write to stdout should succeed");

                    Task::future(async move {
                        ::smol::Timer::after(Duration::from_secs_f32(0.025)).await;
                        Msg::Wave(off + 1.0)
                    })
                }
            }
        }
    }

    /// View game board.
    pub fn view(&self) -> Element<'_, Msg> {
        self.cells
            .iter()
            .enumerate()
            .fold(
                Column::new().padding(3).spacing(3).push(
                    Row::new()
                        .width(Fill)
                        .push(
                            button("Fill")
                                .on_press_with(|| Msg::Fill)
                                .style(widget::button::success),
                        )
                        .push(
                            button("Reset")
                                .on_press_with(|| Msg::Reset)
                                .style(widget::button::danger),
                        )
                        .push(
                            button("Wave")
                                .on_press_with(|| Msg::Wave(1.0))
                                .style(widget::button::secondary),
                        )
                        .push(widget::space::horizontal()),
                ),
                |column, (y, cells)| {
                    column.push(
                        cells
                            .iter()
                            .enumerate()
                            .fold(Row::new().spacing(3), |row, (x, cell)| {
                                row.push(cell.view(x, y, self.is_success))
                            }),
                    )
                },
            )
            .into()
    }
}

/// Application entry.
fn main() -> ::color_eyre::Result<()> {
    let Cli {} = Cli::parse();
    ::color_eyre::install()?;
    ::env_logger::builder()
        .filter_level(LevelFilter::Info)
        .write_style(::env_logger::WriteStyle::Always)
        .init();
    ::log::info!("log initialized");

    ::iced::application(State::default, State::update, State::view)
        .theme(|_: &State| Theme::Light)
        .title(|_: &State| "Match Num".to_owned())
        .run()?;

    Ok(())
}
