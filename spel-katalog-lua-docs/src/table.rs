//! Documentation table display.

use ::core::ops::{BitOr, BitOrAssign};
use ::std::sync::LazyLock;

use ::derive_more::IsVariant;
use ::iced_core::Font;
use ::iced_widget::{self as widget, rich_text, text::Span};
use ::yaml_rust2::Yaml;

use crate::{
    Element, Item, Map, Message, SpanExt, category, empty_spans, indented,
    state::{DocsState, ItemId},
    with_content,
};

/// Keys used when parsing yaml.
#[derive(Debug)]
struct Keys {
    /// Doc comment.
    doc: Yaml,
    /// Fields.
    fields: Yaml,
    /// Parameters.
    params: Yaml,
    /// Parameter.
    param: Yaml,
    /// Return.
    r#return: Yaml,
    /// Returns.
    returns: Yaml,
    /// Union.
    union: Yaml,
    /// Enum.
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

/// Table item.
#[derive(Debug, Clone)]
pub struct Table<S> {
    /// What kind of table to display.
    pub kind: TableKind,
    /// Id in state of this table.
    pub id: ItemId,
    /// Doc comment of table.
    pub doc: Option<S>,
    /// Union variants of table.
    pub union: Vec<Item<S>>,
    /// Fields of table.
    pub fields: Map<S, Item<S>>,
    /// Parameters of table.
    pub params: Map<S, Item<S>>,
    /// Return value of table.
    pub r#return: Vec<Item<S>>,
    /// Enum variants of table.
    pub r#enum: Vec<(S, Option<S>)>,
}

/// Kind of table documentation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, IsVariant)]
pub enum TableKind {
    /// Table has no clear kind.
    #[default]
    None,
    /// Table is only a union.
    Union,
    /// Table is only a table.
    Table,
    /// Table is only a function.
    Function,
    /// Table is only an enum.
    Enum,
    /// Table has multiple kinds.
    Mixed,
}

impl TableKind {
    /// Get name kind or fallback.
    fn name<'a>(self, fallback: impl 'a + FnOnce() -> &'a str) -> &'a str {
        match self {
            TableKind::None => "value",
            TableKind::Union => "union",
            TableKind::Table => "table",
            TableKind::Function => "function",
            TableKind::Enum => "enum",
            TableKind::Mixed => fallback(),
        }
    }
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
    /// Check if table is union.
    const fn is_union(&self) -> bool {
        !self.union.is_empty()
    }

    /// Check if table is an enum.
    const fn is_enum(&self) -> bool {
        !self.r#enum.is_empty()
    }

    /// Check if table is a function.
    fn is_function(&self) -> bool {
        !self.r#return.is_empty() || !self.params.is_empty()
    }

    /// Check if table is a table.
    fn is_table(&self) -> bool {
        !self.fields.is_empty()
    }

    /// Find what exclusive kind this table is.
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

    /// Get default name of this table.
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

    /// View table without name.
    pub fn view_anon<'a>(&'a self, state: &'a DocsState) -> Element<'a, Message> {
        self.view_(None, state)
    }

    /// View table with the given name.
    pub fn view<'a>(&'a self, name: &'a str, state: &'a DocsState) -> Element<'a, Message> {
        self.view_(Some(name), state)
    }

    /// View table with an optional name.
    fn view_<'a>(&'a self, name: Option<&'a str>, state: &'a DocsState) -> Element<'a, Message> {
        if name.is_some() && !state[self.id] {
            return self.view_name_doc(name, self.kind.name(|| self.default_name()));
        }
        match self.kind {
            TableKind::None => self.view_name_doc(name, "value"),
            TableKind::Union => self.view_union(name, state),
            TableKind::Enum => self.view_enum(name),
            TableKind::Table => self.view_table(name, state),
            TableKind::Function | TableKind::Mixed => self.view_mixed(name, state),
        }
    }

    /// Get funtion signature of table.
    fn function_signature<'a, F: From<Font>>(
        &'a self,
        name: Option<&'a str>,
        kind: &'a str,
    ) -> Vec<Span<'a, Message, F>> {
        let mut spans = Vec::with_capacity(3.max(self.params.len() * 2 + 2));
        spans.extend([
            name.fn_name()
                .with_link(|| Message::Toggle(self.id))
                .unwrap_or_else(|| kind.into_span()),
            "(".into_span(),
        ]);

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
        spans
    }

    /// View name and documentation of table.
    fn view_name_doc<'a>(&'a self, name: Option<&'a str>, kind: &'a str) -> Element<'a, Message> {
        let [doc_sep, doc] = self
            .doc
            .as_ref()
            .map(|doc| [" # ", doc.as_ref()])
            .doc()
            .unwrap_or_else(empty_spans);

        if self.is_function() {
            let mut spans = self.function_signature(name, kind);
            spans.extend([doc_sep, doc]);

            rich_text(spans).into()
        } else {
            let [prefix, name] = name
                .name()
                .map(|name| {
                    [
                        " ".into_span(),
                        name.link_maybe((!self.kind.is_none()).then_some(Message::Toggle(self.id))),
                    ]
                })
                .unwrap_or_else(empty_spans);

            rich_text([kind.into_span(), prefix, name, doc_sep, doc]).into()
        }
    }

    /// View name of table.
    fn view_name<'a>(&'a self, name: Option<&'a str>, kind: &'a str) -> Element<'a, Message> {
        if self.is_function() {
            return rich_text(self.function_signature(name, kind)).into();
        }

        let [prefix, name] = name
            .name()
            .map(|name| [" ".into_span(), name.link(Message::Toggle(self.id))])
            .unwrap_or_else(empty_spans);
        rich_text([kind.into_span(), prefix, name]).into()
    }

    /// View documentation of table.
    fn view_docs(&self) -> Option<Element<'_, Message>> {
        self.doc
            .as_ref()
            .map(|docs| rich_text(["# ", docs.as_ref()].doc()).into())
    }

    /// Display an enum value and it's documentation.
    fn view_enum_value_doc<'a>(value: &'a str, doc: Option<&'a str>) -> Element<'a, Message> {
        let [doc_sep, doc] = doc
            .as_ref()
            .map(|doc| [" # ", doc])
            .doc()
            .unwrap_or_else(empty_spans);
        let [l, value, r] = value.dquoted("\"").ty();
        rich_text([l, value, r, doc_sep, doc]).into()
    }

    /// View self as a simplified union.
    fn view_union<'a>(
        &'a self,
        name: Option<&'a str>,
        state: &'a DocsState,
    ) -> Element<'a, Message> {
        widget::Column::new()
            .push(self.view_name(name, "union"))
            .push(indented(self.union.iter().fold(
                widget::Column::new().push_maybe(self.view_docs()),
                |col, item| col.push(item.view_anon(state)),
            )))
            .into()
    }

    /// View self as a simplified enum.
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

    /// View self as a simplified table.
    fn view_table<'a>(
        &'a self,
        name: Option<&'a str>,
        state: &'a DocsState,
    ) -> Element<'a, Message> {
        widget::Column::new()
            .push(self.view_name(name, "table"))
            .push(indented(self.fields.iter().fold(
                widget::Column::new().push_maybe(self.view_docs()),
                |col, (name, item)| col.push(item.view(name.as_ref(), state)),
            )))
            .into()
    }

    /// View self as a mixed table.
    fn view_mixed<'a>(
        &'a self,
        name: Option<&'a str>,
        state: &'a DocsState,
    ) -> Element<'a, Message> {
        widget::Column::new()
            .push(self.view_name(name, self.default_name()))
            .push(indented(
                widget::Column::new()
                    .push_maybe(self.view_docs())
                    .push_maybe(with_content(&self.fields, |fields| {
                        category(
                            "Fields",
                            fields.map(|(name, item)| item.view(name.as_ref(), state)),
                        )
                    }))
                    .push_maybe(with_content(&self.union, |union| {
                        category("Union", union.map(|item| item.view_anon(state)))
                    }))
                    .push_maybe(with_content(&self.r#enum, |e| {
                        category(
                            "Enum",
                            e.map(|(value, doc)| {
                                Self::view_enum_value_doc(
                                    value.as_ref(),
                                    doc.as_ref().map(|doc| doc.as_ref()),
                                )
                            }),
                        )
                    }))
                    .push_maybe(with_content(&self.params, |params| {
                        category(
                            "Parameters",
                            params.map(|(name, item)| item.view(name.as_ref(), state)),
                        )
                    }))
                    .push_maybe(with_content(&self.r#return, |r| {
                        category("Returns", r.map(|item| item.view_anon(state)))
                    })),
            ))
            .into()
    }
}

impl Table<String> {
    /// Create a table from a yaml value.
    pub fn from_yaml(value: Yaml, state: &mut DocsState) -> Result<Self, Yaml> {
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
                    .filter_map(|value| Item::from_yaml(value, state).ok())
                    .collect()
            })
            .unwrap_or_default();

        let fields = table
            .remove(&KEYS.fields)
            .and_then(Yaml::into_hash)
            .map(|hash| {
                hash.into_iter()
                    .filter_map(|(key, value)| {
                        Some((key.into_string()?, Item::from_yaml(value, state).ok()?))
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
                        Some((key.into_string()?, Item::from_yaml(value, state).ok()?))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let r#return = table
            .remove(&KEYS.r#return)
            .or_else(|| table.remove(&KEYS.returns))
            .map(|value| match value {
                value @ Yaml::String(..) => Item::from_yaml(value, state)
                    .map(|item| vec![item])
                    .unwrap_or_default(),
                Yaml::Array(items) => items
                    .into_iter()
                    .filter_map(|value| Item::from_yaml(value, state).ok())
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
        let id = state.new_id();
        let table = Self {
            kind: TableKind::Mixed,
            id,
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
