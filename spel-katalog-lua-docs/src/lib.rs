//! View lua api documentaion in application.

use ::std::sync::LazyLock;

use ::derive_more::From;
use ::iced::{
    Color, Element,
    Length::Fill,
    Task,
    alignment::Vertical,
    widget::{self, horizontal_space, rich_text, text::Span},
};
use ::indexmap::IndexMap;
use ::tap::TryConv;
use ::yaml_rust2::Yaml;

use crate::{
    simple::Simple,
    state::{DocsState, ItemId},
    table::Table,
};

mod simple;
mod span_ext;
mod state;
mod table;

/// Key value map in use by crate.
type Map<K, V> = IndexMap<K, V, ::rustc_hash::FxBuildHasher>;

pub use span_ext::SpanExt;

/// Documentation Item.
#[derive(Debug, Clone, From)]
enum Item<S> {
    /// Simple docs.
    Simple(Simple<S>),
    /// Table docs.
    Table(Table<S>),
}

impl Item<String> {
    /// Get an item from a yaml value.
    pub fn from_yaml(yaml: Yaml, state: &mut DocsState) -> Result<Self, Yaml> {
        let yaml = match Simple::from_yaml(yaml, state) {
            Ok(simple) => return Ok(Self::Simple(simple)),
            Err(yaml) => yaml,
        };

        let yaml = match Table::from_yaml(yaml, state) {
            Ok(table) => return Ok(Self::Table(table)),
            Err(yaml) => yaml,
        };

        Err(yaml)
    }
}

impl<S: AsRef<str>> Item<S> {
    /// View item with the given name.
    pub fn view<'a>(&'a self, name: &'a str, state: &'a DocsState) -> Element<'a, Message> {
        match self {
            Item::Simple(simple) => simple.view(name),
            Item::Table(table) => table.view(name, state),
        }
    }

    /// View item without a name.
    pub fn view_anon<'a>(&'a self, state: &'a DocsState) -> Element<'a, Message> {
        match self {
            Item::Simple(simple) => simple.view_anon(),
            Item::Table(table) => table.view_anon(state),
        }
    }
}

/// View element indented,
fn indented<'a, M: 'a>(elem: impl Into<Element<'a, M>>) -> Element<'a, M> {
    widget::Row::new()
        .align_y(Vertical::Top)
        .push(horizontal_space().width(20))
        .push(elem)
        .into()
}

/// View a category.
fn category<'a, M: 'a>(
    name: &'a str,
    elements: impl IntoIterator<Item = impl Into<Element<'a, M>>>,
) -> Element<'a, M> {
    widget::Column::new()
        .push(name)
        .push(indented(
            elements
                .into_iter()
                .fold(widget::Column::new(), |col, elem| col.push(elem)),
        ))
        .into()
}

/// Use a closure to display an iterable if it has any items.
fn with_content<'a, I, C, F, T: 'a>(content: I, f: F) -> Option<T>
where
    F: FnOnce(::std::iter::Peekable<<I as IntoIterator>::IntoIter>) -> T,
    I: IntoIterator<Item = C>,
{
    let mut peekable = content.into_iter().peekable();
    if peekable.peek().is_some() {
        Some(f(peekable))
    } else {
        None
    }
}

/// Create an array of empty spans.
fn empty_spans<'a, const N: usize, L, F>() -> [Span<'a, L, F>; N] {
    ::std::array::from_fn(|_| Span::new(""))
}

/// Message in use by [DocsViewer].
#[derive(Debug, Clone, Copy)]
pub enum Message {
    /// Toggle display of item.
    Toggle(ItemId),
    /// Set expanded state of all items.
    SetAll(bool),
}

/// Documentation viewer.
#[derive(Debug)]
pub struct DocsViewer {
    /// Top-level item docs.
    docs: Map<String, Item<String>>,
    /// Display state of docs.
    state: DocsState,
}

impl Default for DocsViewer {
    fn default() -> Self {
        static DOCUMENT: LazyLock<(Map<String, Item<String>>, DocsState)> = LazyLock::new(|| {
            let [document] =
                ::yaml_rust2::YamlLoader::load_from_str(include_str!("../../lua/docs.yml"))
                    .expect("embedded lua docs should be valid yaml")
                    .try_conv::<[Yaml; 1]>()
                    .expect("embedded lua docs yaml should be a single yaml document");
            let mut state = DocsState::default();
            let docs = document
                .into_hash()
                .map(|hash| {
                    hash.into_iter()
                        .filter_map(|(key, value)| {
                            Some((key.into_string()?, Item::from_yaml(value, &mut state).ok()?))
                        })
                        .collect()
                })
                .unwrap_or_default();
            (docs, state)
        });
        let (docs, state) = (*DOCUMENT).clone();
        Self { docs, state }
    }
}

impl DocsViewer {
    /// Update application state based on message.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Toggle(item_id) => {
                self.state[item_id] = !self.state[item_id];
                Task::none()
            }
            Message::SetAll(value) => {
                self.state.set_all(value);
                Task::none()
            }
        }
    }

    /// Render documentaion viewer as an element.
    pub fn view(&self) -> Element<'_, Message> {
        widget::scrollable(
            self.docs
                .iter()
                .fold(
                    widget::Column::new().push(rich_text(
                        [
                            "Show All".with_link(|| Message::SetAll(true)),
                            " | ".into_span(),
                            "Hide All".with_link(|| Message::SetAll(false)),
                        ]
                        .with_color(Color::from_rgb(0.7, 0.7, 0.7)),
                    )),
                    |col, (key, value)| col.push(value.view(key, &self.state)),
                )
                .spacing(5)
                .width(Fill)
                .padding(5),
        )
        .into()
    }
}
