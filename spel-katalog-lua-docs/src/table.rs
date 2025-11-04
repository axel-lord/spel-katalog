use ::std::sync::LazyLock;

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

#[derive(Debug, Clone)]
pub struct Table<S> {
    pub doc: Option<S>,
    pub union: Vec<Item<S>>,
    pub fields: Map<S, Item<S>>,
    pub params: Map<S, Item<S>>,
    pub r#return: Vec<Item<S>>,
    pub r#enum: Vec<(S, Option<S>)>,
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

    fn is_empty(&self) -> bool {
        self.union.is_empty()
            && self.params.is_empty()
            && self.r#return.is_empty()
            && self.r#enum.is_empty()
            && self.fields.is_empty()
    }

    pub fn view<'a>(&'a self, name: &'a str) -> Element<'a, Message> {
        if self.is_empty() {
            let [doc_sep, doc] = self
                .doc
                .as_ref()
                .map(|doc| [" # ", doc.as_ref()])
                .doc()
                .unwrap_or_else(empty_spans);
            return rich_text([name.name(), doc_sep, doc]).into();
        }

        let Self {
            doc,
            union,
            fields,
            params,
            r#return,
            r#enum,
        } = self;

        let name = if self.is_function() {
            let mut spans = Vec::with_capacity(3.max(self.params.len() * 2 + 2));
            spans.extend([name.name(), "(".into_span()]);

            let mut iter = self.params.keys();

            if let Some(first) = iter.next() {
                spans.push(first.as_ref().param());
            };

            for param in iter {
                spans.push(", ".into_span());
                spans.push(param.as_ref().param());
            }

            spans.push("):".into_span());
            rich_text(spans)
        } else {
            rich_text([name.name(), ":".into_span()])
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
                            let [doc_sep, doc] = doc
                                .as_ref()
                                .map(|doc| [" # ", doc.as_ref()])
                                .doc()
                                .unwrap_or_else(empty_spans);
                            let [l, value, r] = value.as_ref().dquoted("\"").ty();
                            col.push(rich_text([l, value, r, doc_sep, doc]))
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

    pub fn view_anon(&self) -> Element<'_, Message> {
        self.view(if self.is_union() {
            "union"
        } else if self.is_enum() {
            "enum"
        } else if self.fields.is_empty() && self.is_function() {
            "function"
        } else {
            "table"
        })
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
        Ok(Self {
            doc,
            union,
            fields,
            params,
            r#return,
            r#enum,
        })
    }
}
