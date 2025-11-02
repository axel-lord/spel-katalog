//! View lua api documentaion in application.

use ::std::sync::LazyLock;

use ::iced::{
    Color, Element,
    Length::Fill,
    Task,
    alignment::{Horizontal, Vertical},
    widget::{self, horizontal_space, text::Span},
};
use ::indexmap::IndexMap;
use ::tap::TryConv;
use ::yaml_rust2::Yaml;

type Map<K, V> = IndexMap<K, V, ::rustc_hash::FxBuildHasher>;

#[derive(Debug)]
struct Keys {
    doc: Yaml,
    fields: Yaml,
    params: Yaml,
    r#return: Yaml,
    union: Yaml,
    r#enum: Yaml,
}

static KEYS: LazyLock<Keys> = LazyLock::new(|| Keys {
    doc: Yaml::String("doc".to_owned()),
    fields: Yaml::String("fields".to_owned()),
    params: Yaml::String("params".to_owned()),
    r#return: Yaml::String("return".to_owned()),
    union: Yaml::String("union".to_owned()),
    r#enum: Yaml::String("enum".to_owned()),
});

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

#[derive(Debug, Clone)]
enum Item {
    Simple {
        doc: Option<String>,
        ty: String,
        attr: Attr,
    },
    Table {
        doc: Option<String>,
        union: Vec<Item>,
        fields: Map<String, Item>,
        params: Map<String, Item>,
        r#return: Vec<Item>,
        r#enum: Vec<(String, Option<String>)>,
    },
}

impl Item {
    fn from_yaml(value: Yaml) -> Option<Self> {
        match value {
            Yaml::String(ty) => {
                let (ty, attr) = Attr::split_ty(ty);
                Some(Self::Simple {
                    doc: None,
                    ty,
                    attr,
                })
            }
            Yaml::Array(yamls) => match <[Yaml; 2]>::try_from(yamls).ok()? {
                [Yaml::String(ty), Yaml::String(doc)] => {
                    let (ty, attr) = Attr::split_ty(ty);
                    let doc = Some(doc);
                    Some(Self::Simple { doc, ty, attr })
                }
                _ => None,
            },
            Yaml::Hash(mut table) => {
                let doc = table.remove(&KEYS.doc).and_then(Yaml::into_string);

                let union = table
                    .remove(&KEYS.union)
                    .and_then(Yaml::into_vec)
                    .map(|v| v.into_iter().filter_map(Self::from_yaml).collect())
                    .unwrap_or_default();

                let fields = table
                    .remove(&KEYS.fields)
                    .and_then(Yaml::into_hash)
                    .map(|hash| {
                        hash.into_iter()
                            .filter_map(|(key, value)| {
                                Some((key.into_string()?, Self::from_yaml(value)?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let params = table
                    .remove(&KEYS.params)
                    .and_then(Yaml::into_hash)
                    .map(|hash| {
                        hash.into_iter()
                            .filter_map(|(key, value)| {
                                Some((key.into_string()?, Self::from_yaml(value)?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let r#return = table
                    .remove(&KEYS.r#return)
                    .map(|value| match value {
                        value @ Yaml::String(..) => Self::from_yaml(value)
                            .map(|item| vec![item])
                            .unwrap_or_default(),
                        Yaml::Array(items) => {
                            items.into_iter().filter_map(Self::from_yaml).collect()
                        }
                        _ => Vec::new(),
                    })
                    .unwrap_or_default();

                let r#enum = table
                    .remove(&KEYS.r#enum)
                    .and_then(Yaml::into_vec)
                    .map(|values| {
                        let conv_value = |value: Yaml| match value {
                            Yaml::Real(s) | Yaml::String(s) => Some(s),
                            Yaml::Integer(n) => Some(n.to_string()),
                            Yaml::Boolean(b) => Some(b.to_string()),
                            _ => None,
                        };
                        values
                            .into_iter()
                            .filter_map(|value| match value {
                                Yaml::Array(arr) => {
                                    let [value, doc] = <[Yaml; 2]>::try_from(arr).ok()?;
                                    let doc = doc.into_string()?;
                                    let value = conv_value(value)?;

                                    Some((value, Some(doc)))
                                }
                                value => Some((conv_value(value)?, None)),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                Some(Self::Table {
                    doc,
                    union,
                    fields,
                    params,
                    r#return,
                    r#enum,
                })
            }
            _ => None,
        }
    }

    pub fn view<'a>(&'a self, name: &'a str) -> Element<'a, Message> {
        let name = Span::new(name);
        match self {
            Item::Simple { doc, ty, attr: _ } => {
                let ty = Span::new(ty).color(Color::new(0.5, 1.0, 0.5, 1.0));
                if let Some(doc) = doc {
                    let doc = Span::new(doc);
                    widget::rich_text([name, Span::new(": "), ty, Span::new(" "), doc]).into()
                } else {
                    widget::rich_text([name, Span::new(": "), ty]).into()
                }
            }
            Item::Table {
                doc,
                union,
                fields,
                params,
                r#return,
                r#enum,
            } => {
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
                            Some((key.into_string()?, Item::from_yaml(value)?))
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
        widget::container("DocsViewer").center(Fill).into()
    }
}
