use ::std::{
    ops::{BitOr, BitOrAssign},
    sync::LazyLock,
};

use ::derive_more::IsVariant;
use ::iced::{
    Element,
    widget::{self, rich_text},
};
use ::yaml_rust2::Yaml;

use crate::{Item, Map, Message, SpanExt, empty_spans, indented, with_content};

#[derive(Debug)]
struct Keys {
    doc: Yaml,
    fields: Yaml,
    params: Yaml,
    param: Yaml,
    r#return: Yaml,
    returns: Yaml,
    union: Yaml,
    r#enum: Yaml,
}

static KEYS: LazyLock<Keys> = LazyLock::new(|| Keys {
    doc: Yaml::String("doc".to_owned()),
    fields: Yaml::String("fields".to_owned()),
    param: Yaml::String("param".to_owned()),
    params: Yaml::String("params".to_owned()),
    r#return: Yaml::String("return".to_owned()),
    returns: Yaml::String("returns".to_owned()),
    union: Yaml::String("union".to_owned()),
    r#enum: Yaml::String("enum".to_owned()),
});

#[derive(Debug, Clone)]
pub struct Table<S> {
    pub kind: TableKind,
    pub doc: Option<S>,
    pub union: Vec<Item<S>>,
    pub fields: Map<S, Item<S>>,
    pub params: Map<S, Item<S>>,
    pub r#return: Vec<Item<S>>,
    pub r#enum: Vec<(S, Option<S>)>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, IsVariant)]
pub enum TableKind {
    #[default]
    None,
    Union,
    Table,
    Function,
    Enum,
    Mixed,
}

impl BitOr for TableKind {
    type Output = TableKind;

    fn bitor(self, rhs: Self) -> Self::Output {
        if self.is_none() {
            rhs
        } else if self == rhs {
            self
        } else {
            Self::Mixed
        }
    }
}

impl BitOrAssign for TableKind {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl<S: AsRef<str>> Table<S> {
    fn is_union(&self) -> bool {
        !self.union.is_empty()
    }

    fn is_enum(&self) -> bool {
        !self.r#enum.is_empty()
    }

    fn is_function(&self) -> bool {
        !self.r#return.is_empty() || !self.params.is_empty()
    }

    fn is_table(&self) -> bool {
        !self.fields.is_empty()
    }

    fn find_kind(&self) -> TableKind {
        let mut kind = TableKind::None;

        if self.is_union() {
            kind |= TableKind::Union;
        }

        if self.is_enum() {
            kind |= TableKind::Enum;
        }

        if self.is_function() {
            kind |= TableKind::Function;
        }

        if self.is_table() {
            kind |= TableKind::Table;
        }

        kind
    }

    fn default_name(&self) -> &'static str {
        if self.is_union() {
            "union"
        } else if self.is_enum() {
            "enum"
        } else if self.fields.is_empty() && self.is_function() {
            "function"
        } else {
            "table"
        }
    }

    pub fn view_anon(&self) -> Element<'_, Message> {
        self.view_(None)
    }

    pub fn view<'a>(&'a self, name: &'a str) -> Element<'a, Message> {
        self.view_(Some(name))
    }

    fn view_<'a>(&'a self, name: Option<&'a str>) -> Element<'a, Message> {
        match self.kind {
            TableKind::None => self.view_name_doc(name, "value"),
            TableKind::Union => self.view_union(name),
            TableKind::Enum => self.view_enum(name),
            TableKind::Table => self.view_table(name),
            TableKind::Function | TableKind::Mixed => {
                self.view_mixed(name.unwrap_or_else(|| self.default_name()))
            }
        }
    }

    fn view_name_doc<'a>(&'a self, name: Option<&'a str>, kind: &'a str) -> Element<'a, Message> {
        let [prefix, name] = name
            .name()
            .map(|name| [" ".into_span(), name])
            .unwrap_or_else(empty_spans);
        let [doc_sep, doc] = self
            .doc
            .as_ref()
            .map(|doc| [" # ", doc.as_ref()])
            .doc()
            .unwrap_or_else(empty_spans);

        rich_text([kind.into_span(), prefix, name, doc_sep, doc]).into()
    }

    fn view_name<'a>(&'a self, name: Option<&'a str>, kind: &'a str) -> Element<'a, Message> {
        let [prefix, name] = name
            .name()
            .map(|name| [" ".into_span(), name])
            .unwrap_or_else(empty_spans);
        rich_text([kind.into_span(), prefix, name]).into()
    }

    fn view_docs(&self) -> Option<Element<'_, Message>> {
        self.doc
            .as_ref()
            .map(|docs| rich_text(["# ", docs.as_ref()].doc()).into())
    }

    fn view_enum_value_doc<'a>(value: &'a str, doc: Option<&'a str>) -> Element<'a, Message> {
        let [doc_sep, doc] = doc
            .as_ref()
            .map(|doc| [" # ", doc])
            .doc()
            .unwrap_or_else(empty_spans);
        let [l, value, r] = value.dquoted("\"").ty();
        rich_text([l, value, r, doc_sep, doc]).into()
    }

    fn view_union<'a>(&'a self, name: Option<&'a str>) -> Element<'a, Message> {
        widget::Column::new()
            .push(self.view_name(name, "union"))
            .push(indented(self.union.iter().fold(
                widget::Column::new().push_maybe(self.view_docs()),
                |col, item| col.push(item.view_anon()),
            )))
            .into()
    }

    fn view_enum<'a>(&'a self, name: Option<&'a str>) -> Element<'a, Message> {
        widget::Column::new()
            .push(self.view_name(name, "enum"))
            .push(indented(self.r#enum.iter().fold(
                widget::Column::new().push_maybe(self.view_docs()),
                |col, (value, doc)| {
                    col.push(Self::view_enum_value_doc(
                        value.as_ref(),
                        doc.as_ref().map(|doc| doc.as_ref()),
                    ))
                },
            )))
            .into()
    }

    fn view_table<'a>(&'a self, name: Option<&'a str>) -> Element<'a, Message> {
        widget::Column::new()
            .push(self.view_name(name, "table"))
            .push(indented(self.fields.iter().fold(
                widget::Column::new().push_maybe(self.view_docs()),
                |col, (name, item)| col.push(item.view(name.as_ref())),
            )))
            .into()
    }

    fn view_mixed<'a>(&'a self, name: &'a str) -> Element<'a, Message> {
        let Self {
            kind: _,
            doc,
            union,
            fields,
            params,
            r#return,
            r#enum,
        } = self;

        let name = if self.is_function() {
            let mut spans = Vec::with_capacity(3.max(self.params.len() * 2 + 2));
            spans.extend([name.fn_name(), "(".into_span()]);

            let mut iter = self.params.keys();

            if let Some(first) = iter.next() {
                let first = first.as_ref();

                spans.push(match first {
                    "class" => "class".class_param(),
                    "self" => "self".self_param(),
                    other => other.param(),
                })
            };

            for param in iter {
                spans.push(", ".into_span());
                spans.push(param.as_ref().param());
            }

            spans.push(")".into_span());
            rich_text(spans)
        } else {
            rich_text([
                self.default_name().into_span(),
                " ".into_span(),
                name.name(),
            ])
        };

        widget::Column::new()
            .push(name)
            .push(indented(
                widget::Column::new()
                    .push_maybe(
                        doc.as_ref()
                            .map(|doc| rich_text(["# ", doc.as_ref()].doc())),
                    )
                    .push_maybe(with_content(fields, |_| "Fields"))
                    .push_maybe(with_content(fields, |fields| {
                        indented(fields.fold(widget::Column::new(), |col, (name, item)| {
                            col.push(item.view(name.as_ref()))
                        }))
                    }))
                    .push_maybe(with_content(union, |_| "Union"))
                    .push_maybe(with_content(union, |union| {
                        indented(union.fold(widget::Column::new(), |col, item| {
                            col.push(item.view_anon())
                        }))
                    }))
                    .push_maybe(with_content(r#enum, |_| "Enum"))
                    .push_maybe(with_content(r#enum, |r#enum| {
                        indented(r#enum.fold(widget::Column::new(), |col, (value, doc)| {
                            col.push(Self::view_enum_value_doc(
                                value.as_ref(),
                                doc.as_ref().map(|doc| doc.as_ref()),
                            ))
                        }))
                    }))
                    .push_maybe(with_content(params, |_| "Parameters"))
                    .push_maybe(with_content(params, |params| {
                        indented(params.fold(widget::Column::new(), |col, (name, item)| {
                            col.push(item.view(name.as_ref()))
                        }))
                    }))
                    .push_maybe(with_content(r#return, |_| "Returns"))
                    .push_maybe(with_content(r#return, |r#return| {
                        indented(r#return.fold(widget::Column::new(), |col, item| {
                            col.push(item.view_anon())
                        }))
                    })),
            ))
            .into()
    }
}

impl TryFrom<Yaml> for Table<String> {
    type Error = Yaml;

    fn try_from(value: Yaml) -> Result<Self, Self::Error> {
        let mut table = match value {
            Yaml::Hash(hash) => hash,
            other => return Err(other),
        };
        let doc = table.remove(&KEYS.doc).and_then(Yaml::into_string);

        let union = table
            .remove(&KEYS.union)
            .and_then(Yaml::into_vec)
            .map(|v| {
                v.into_iter()
                    .filter_map(|value| Item::try_from(value).ok())
                    .collect()
            })
            .unwrap_or_default();

        let fields = table
            .remove(&KEYS.fields)
            .and_then(Yaml::into_hash)
            .map(|hash| {
                hash.into_iter()
                    .filter_map(|(key, value)| {
                        Some((key.into_string()?, Item::try_from(value).ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let params = table
            .remove(&KEYS.params)
            .or_else(|| table.remove(&KEYS.param))
            .and_then(Yaml::into_hash)
            .map(|hash| {
                hash.into_iter()
                    .filter_map(|(key, value)| {
                        Some((key.into_string()?, Item::try_from(value).ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let r#return = table
            .remove(&KEYS.r#return)
            .or_else(|| table.remove(&KEYS.returns))
            .map(|value| match value {
                value @ Yaml::String(..) => Item::try_from(value)
                    .map(|item| vec![item])
                    .unwrap_or_default(),
                Yaml::Array(items) => items
                    .into_iter()
                    .filter_map(|value| Item::try_from(value).ok())
                    .collect(),
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
        let table = Self {
            kind: TableKind::Mixed,
            doc,
            union,
            fields,
            params,
            r#return,
            r#enum,
        };
        let kind = table.find_kind();
        Ok(Self { kind, ..table })
    }
}
