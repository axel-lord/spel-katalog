//! View lua api documentaion in application.

use ::std::sync::LazyLock;

use ::derive_more::From;
use ::iced::{
    Color, Element, Task,
    alignment::{Horizontal, Vertical},
    widget::{self, horizontal_space, text::Span},
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
enum Item {
    Simple(Simple),
    Table(Table),
}

impl TryFrom<Yaml> for Item {
    type Error = Yaml;

    fn try_from(value: Yaml) -> Result<Self, Self::Error> {
        Simple::try_from(value)
            .map(Item::Simple)
            .or_else(|value| Table::try_from(value).map(Item::Table))
    }
}

impl Item {
    pub fn view<'a>(&'a self, name: &'a str) -> Element<'a, Message> {
        match self {
            Item::Simple(simple) => simple.view(name),
            Item::Table(Table {
                doc,
                union,
                fields,
                params,
                r#return,
                r#enum,
            }) => {
                let name = Span::new(name);
                widget::Column::new()
                    .align_x(Horizontal::Left)
                    .push(widget::rich_text([name, Span::new(": ")]))
                    .push(indented(
                        widget::Column::new()
                            .push_maybe(doc.as_ref().map(String::as_str))
                            .push_maybe((!union.is_empty()).then_some("Union"))
                            .push_maybe((!union.is_empty()).then(|| {
                                indented(union.iter().fold(widget::Column::new(), |col, item| {
                                    col.push(item.view(""))
                                }))
                            }))
                            .push_maybe((!fields.is_empty()).then_some("Fields"))
                            .push_maybe((!fields.is_empty()).then(|| {
                                indented(
                                    fields
                                        .iter()
                                        .fold(widget::Column::new(), |col, (key, value)| {
                                            col.push(value.view(&key))
                                        }),
                                )
                            })),
                    ))
                    .into()
            }
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

/// Message in use by [DocsViewer].
#[derive(Debug, Clone)]
pub enum Message {}

/// Documentation viewer.
#[derive(Debug)]
pub struct DocsViewer {
    docs: Map<String, Item>,
}

impl Default for DocsViewer {
    fn default() -> Self {
        static DOCUMENT: LazyLock<Map<String, Item>> = LazyLock::new(|| {
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
