use ::iced::{
    Element,
    theme::Palette,
    widget::{
        self,
        markdown::{Item, Url},
    },
};

pub fn view(md: &[Item], palette: Palette) -> Element<'_, Url> {
    widget::container(widget::scrollable(widget::markdown(
        md,
        widget::markdown::Settings {
            text_size: 14.into(),
            h1_size: 24.into(),
            h2_size: 22.into(),
            h3_size: 20.into(),
            h4_size: 18.into(),
            h5_size: 16.into(),
            h6_size: 16.into(),
            code_size: 14.into(),
        },
        widget::markdown::Style::from_palette(palette),
    )))
    .padding(5)
    .into()
}
