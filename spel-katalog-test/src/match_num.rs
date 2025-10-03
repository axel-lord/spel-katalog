//! Sample game, should run on all platforms, and log with color.

use ::clap::Parser;
use ::iced::{
    Element,
    Length::Fill,
    Task,
    widget::{self, Button, Column, Row, button, horizontal_space, text},
};
use ::log::LevelFilter;
use ::rand::{Rng, seq::SliceRandom};
use ::tap::Tap;

#[derive(Debug, Parser)]
#[command(author, version, long_about = None)]
struct Cli {}

#[derive(Debug, Clone, Copy)]
enum Msg {
    Pressed { x: usize, y: usize },
    Fill,
    Reset,
}

#[derive(Debug, Clone)]
struct Cell {
    is_selected: bool,
    value: String,
}

impl Cell {
    pub fn view(&self, x: usize, y: usize, is_success: bool) -> Button<'_, Msg> {
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

#[derive(Debug, Default)]
struct State {
    cells: Vec<Vec<Cell>>,
    dim: usize,
    dupe: String,
    is_success: bool,
}

impl State {
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

    fn get_selected(&self) -> Vec<&str> {
        let mut v = Vec::new();

        for cell in self.cells.iter().flatten() {
            if cell.is_selected {
                v.push(cell.value.as_str());
            }
        }

        v
    }

    pub fn update(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::Pressed { x, y } => {
                let cell = &mut self.cells[y][x];
                cell.is_selected = !cell.is_selected;

                ::log::info!("tile ({x}, {y}) pressed");

                let selected = self.get_selected();

                self.is_success = selected.as_slice() == &[self.dupe.as_str(), self.dupe.as_str()];

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
        }
    }

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
                        .push(horizontal_space()),
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
    ::iced::run("Find Pair", State::update, State::view)?;
    Ok(())
}
