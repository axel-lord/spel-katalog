//! Setting viewer helpers.

use ::iced_core::{Alignment, Element};
use ::iced_widget::{Column, Row, container, pick_list, text_input};
use ::tap::Pipe;
use spel_katalog_common::w;

use crate::{DefaultStr, Title, Variants};

/// Create a choice widget for a setting.
pub fn enum_choice<'a, T, M>(
    value: Option<T>,
) -> (
    &'a str,
    Element<'a, M, ::iced_core::Theme, ::iced_renderer::Renderer>,
)
where
    T: Variants + Clone + PartialEq + ToString + Default + Title,
    M: 'a + From<T>,
{
    (
        T::title(),
        pick_list(
            T::VARIANTS,
            Some(value.unwrap_or_default()),
            ::core::convert::identity,
        )
        .padding(3)
        .pipe(Element::from)
        .map(M::from),
    )
}

/// Create a list of settings consisting of name and set columns.
pub fn enum_list<'a, M>(
    settings: impl IntoIterator<
        Item = (
            &'a str,
            Element<'a, M, ::iced_core::Theme, ::iced_renderer::Renderer>,
        ),
    >,
) -> Row<'a, M>
where
    M: 'a,
{
    let mut l = w::col().align_x(Alignment::End);
    let mut r = w::col().align_x(Alignment::Start);

    for (name, elem) in settings {
        l = l.push(container(name).padding(3));
        r = r.push(elem);
    }

    w::row().align_y(Alignment::Start).push(l).push(r)
}

/// Create a path input.
pub fn path_input<'a, T, M>(
    value: &Option<T>,
) -> (
    &'a str,
    Element<'a, M, ::iced_core::Theme, ::iced_renderer::Renderer>,
)
where
    T: 'static + DefaultStr + AsRef<str> + From<String> + Clone + Title,
    M: 'a + From<T>,
{
    (
        T::title(),
        text_input(
            T::default_str(),
            match value {
                None => "",
                Some(value) => value.as_ref(),
            },
        )
        .padding(3)
        .on_input(T::from)
        .pipe(Element::from)
        .map(M::from),
    )
}

/// Create a list of settings consisting of name and input columns.
pub fn path_list<'a, M>(
    settings: impl IntoIterator<
        Item = (
            &'a str,
            Element<'a, M, ::iced_core::Theme, ::iced_renderer::Renderer>,
        ),
    >,
) -> Column<'a, M, ::iced_core::Theme, ::iced_renderer::Renderer>
where
    M: 'a,
{
    let mut col = w::col().align_x(Alignment::Start);

    for (name, elem) in settings {
        col = col.push(name).push(elem)
    }

    col
}
