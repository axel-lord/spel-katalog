//! View lua api documentaion in application.

use ::std::sync::LazyLock;

use ::derive_more::From;
use ::iced::{
    Element, Task,
    alignment::Vertical,
    widget::{self, horizontal_space},
};
use ::indexmap::IndexMap;
use ::tap::TryConv;
use ::yaml_rust2::Yaml;

use crate::{simple::Simple, table::Table};

mod simple;
mod table;

type Map<K, V> = IndexMap<K, V, ::rustc_hash::FxBuildHasher>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Attr {
    #[default]
    None,
    Optional,
    Variadic,
}

impl Attr {
    pub fn split_ty(mut ty: String) -> (String, Attr) {
        let attr = if ty.ends_with('?') {
            _ = ty.pop();
            Attr::Optional
        } else if ty.ends_with("...") {
            (0..3).for_each(|_| _ = ty.pop());
            Attr::Variadic
        } else {
            Attr::None
        };
        (ty, attr)
    }
}

#[derive(Debug, Clone, From)]
enum Item<S> {
    Simple(Simple<S>),
    Table(Table<S>),
}

impl TryFrom<Yaml> for Item<String> {
    type Error = Yaml;

    fn try_from(value: Yaml) -> Result<Self, Self::Error> {
        Simple::try_from(value)
            .map(Item::Simple)
            .or_else(|value| Table::try_from(value).map(Item::Table))
    }
}

impl<S: AsRef<str>> Item<S> {
    pub fn view<'a>(&'a self, name: &'a str) -> Element<'a, Message> {
        match self {
            Item::Simple(simple) => simple.view(name),
            Item::Table(table) => table.view(name),
        }
    }

    pub fn view_anon(&self) -> Element<'_, Message> {
        match self {
            Item::Simple(simple) => simple.view_anon(),
            Item::Table(table) => table.view_anon(),
        }
    }
}

fn indented<'a, M: 'a>(elem: impl Into<Element<'a, M>>) -> Element<'a, M> {
    widget::Row::new()
        .align_y(Vertical::Top)
        .push(horizontal_space().width(20))
        .push(elem)
        .into()
}

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

/// Message in use by [DocsViewer].
#[derive(Debug, Clone)]
pub enum Message {}

/// Documentation viewer.
#[derive(Debug)]
pub struct DocsViewer {
    docs: Map<String, Item<String>>,
}

impl Default for DocsViewer {
    fn default() -> Self {
        static DOCUMENT: LazyLock<Map<String, Item<String>>> = LazyLock::new(|| {
            let [document] =
                ::yaml_rust2::YamlLoader::load_from_str(include_str!("../../lua/docs.yml"))
                    .expect("embedded lua docs should be valid yaml")
                    .try_conv::<[Yaml; 1]>()
                    .expect("embedded lua docs yaml should be a single yaml document");
            document
                .into_hash()
                .map(|hash| {
                    hash.into_iter()
                        .filter_map(|(key, value)| {
                            Some((key.into_string()?, Item::try_from(value).ok()?))
                        })
                        .collect()
                })
                .unwrap_or_default()
        });
        Self {
            docs: DOCUMENT.clone(),
        }
    }
}

impl DocsViewer {
    /// Update application state based on message.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {}
    }

    /// Render documentaion viewer as an element.
    pub fn view(&self) -> Element<'_, Message> {
        self.docs
            .iter()
            .fold(widget::Column::new(), |col, (key, value)| {
                col.push(value.view(&key))
            })
            .into()
    }
}
