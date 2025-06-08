use ::iced::{
    Alignment::Center,
    widget::{Column, Row},
};

pub fn col<'a, M>() -> Column<'a, M> {
    Column::new().spacing(3)
}

pub fn row<'a, M>() -> Row<'a, M> {
    Row::new().spacing(3).align_y(Center)
}

