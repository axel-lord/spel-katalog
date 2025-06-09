use ::iced::widget;

pub fn box_border(theme: &::iced::Theme) -> widget::container::Style {
    let mut base = widget::container::bordered_box(theme);

    base.background = None;

    base
}
