//! Setting viewer helpers.

use ::iced::{
    Alignment, Element,
    widget::{Column, Row, container, pick_list, text_input},
};
use ::tap::Conv;
use spel_katalog_common::w;

use crate::{DefaultStr, Title, Variants};

/// Create a choice widget for a setting.
pub fn enum_choice<'a, T, M>(value: Option<T>) -> (&'a str, Element<'a, M>)
where
    T: Variants + Clone + PartialEq + ToString + Default + Title,
    M: 'a + From<T>,
{
    (
        T::title(),
        pick_list(
            T::VARIANTS,
            Some(value.unwrap_or_default()),
            ::std::convert::identity,
        )
        .padding(3)
        .conv::<Element<'a, T>>()
        .map(M::from),
    )
}

/// Create a list of settings consisting of name and set columns.
pub fn enum_list<'a, M>(settings: impl IntoIterator<Item = (&'a str, Element<'a, M>)>) -> Row<'a, M>
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
pub fn path_input<'a, T, M>(value: &Option<T>) -> (&'a str, Element<'a, M>)
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
        .conv::<Element<'a, T>>()
        .map(M::from),
    )
}

/// Create a list of settings consisting of name and input columns.
pub fn path_list<'a, M>(
    settings: impl IntoIterator<Item = (&'a str, Element<'a, M>)>,
) -> Column<'a, M>
where
    M: 'a,
{
    let mut col = w::col().align_x(Alignment::Start);

    for (name, elem) in settings {
        col = col.push(name).push(elem)
    }

    col
}
