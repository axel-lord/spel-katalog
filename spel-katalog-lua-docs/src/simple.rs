use ::iced::{
    Element,
    widget::{rich_text, text::Span},
};
use ::itertools::{
    Either::{Left, Right},
    Itertools, chain,
};
use ::yaml_rust2::Yaml;

use crate::{Message, SpanExt, empty_spans};

#[derive(Debug, Clone)]
pub struct Simple<S> {
    pub doc: Option<S>,
    pub ty: SimpleTy<S>,
}

#[derive(Debug, Clone)]
pub enum SimpleTy<S> {
    Single(S, Attr),
    Tuple(Vec<(S, Attr)>),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Attr {
    #[default]
    None,
    Optional,
    Variadic,
    Array,
}

impl Attr {
    pub fn as_str<'a>(self) -> &'a str {
        match self {
            Attr::None => "",
            Attr::Optional => "?",
            Attr::Variadic => "...",
            Attr::Array => "[]",
        }
    }
    pub fn split_ty(mut ty: String) -> (String, Attr) {
        let (attr, len) = if let Some(prefix) = ty.strip_suffix("?") {
            (Attr::Optional, prefix.len())
        } else if let Some(prefix) = ty.strip_suffix("...") {
            (Attr::Variadic, prefix.len())
        } else if let Some(prefix) = ty.strip_suffix("[]") {
            (Attr::Array, prefix.len())
        } else {
            (Attr::None, ty.len())
        };
        ty.drain(len..);
        (ty, attr)
    }
}

impl<S: AsRef<str>> Simple<S> {
    fn view_<'a>(&'a self, name: Option<&'a str>) -> Element<'a, Message> {
        let Self { doc, ty } = self;
        let [name, sep] = name
            .map(|name| [name.name(), " ".into_span()])
            .unwrap_or_else(empty_spans);
        let [doc_sep, doc] = doc
            .as_ref()
            .map(|doc| [" # ", doc.as_ref()])
            .doc()
            .unwrap_or_else(empty_spans);

        match ty {
            SimpleTy::Single(ty, attr) => {
                let ty = ty.as_ref().ty();
                let attr = attr.as_str().into_span();
                rich_text([name, sep, ty, attr, doc_sep, doc]).into()
            }
            SimpleTy::Tuple(items) => {
                let prefix = [name, sep];
                let suffix = [doc_sep, doc];
                let ty = Itertools::intersperse_with(
                    items
                        .iter()
                        .map(|(ty, attr)| [ty.as_ref().ty(), attr.as_str().into_span()]),
                    || [Span::new(", "), Span::new("")],
                )
                .flatten();

                let mut spans = Vec::with_capacity(prefix.len() + suffix.len() + items.len());
                spans.extend(chain!(prefix, ty, suffix));

                rich_text(spans).into()
            }
        }
    }

    pub fn view_anon(&self) -> Element<'_, Message> {
        self.view_(None)
    }

    pub fn view<'a>(&'a self, name: &'a str) -> Element<'a, Message> {
        self.view_(Some(name))
    }
}

impl TryFrom<Yaml> for Simple<String> {
    type Error = Yaml;

    fn try_from(value: Yaml) -> Result<Self, Self::Error> {
        fn split_array(array: Vec<Yaml>) -> Result<Vec<(String, Attr)>, Vec<Yaml>> {
            let (array, types): (Vec<_>, Vec<_>) =
                array.into_iter().partition_map(|yaml| match yaml {
                    Yaml::String(s) => Right(Attr::split_ty(s)),
                    other => Left(other),
                });

            if types.is_empty() {
                return Err(array);
            }

            Ok(types)
        }
        match value {
            Yaml::String(ty) => {
                let (ty, attr) = Attr::split_ty(ty);
                Ok(Self {
                    doc: None,
                    ty: SimpleTy::Single(ty, attr),
                })
            }
            Yaml::Array(yaml) => match yaml.as_slice() {
                [Yaml::String(..), Yaml::String(..)] => {
                    match <[Yaml; 2]>::try_from(yaml).map_err(Yaml::Array)? {
                        [Yaml::String(ty), Yaml::String(doc)] => {
                            let (ty, attr) = Attr::split_ty(ty);
                            let doc = Some(doc);
                            Ok(Self {
                                ty: SimpleTy::Single(ty, attr),
                                doc,
                            })
                        }
                        other => Err(Yaml::Array(Vec::from(other))),
                    }
                }
                [Yaml::Array(..), Yaml::String(..)] => {
                    match <[Yaml; 2]>::try_from(yaml).map_err(Yaml::Array)? {
                        [Yaml::Array(array), Yaml::String(docs)] => {
                            let types = match split_array(array) {
                                Ok(types) => types,
                                Err(array) => {
                                    return Err(Yaml::Array(
                                        [Yaml::Array(array), Yaml::String(docs)].into(),
                                    ));
                                }
                            };
                            Ok(Self {
                                doc: Some(docs),
                                ty: SimpleTy::Tuple(types),
                            })
                        }
                        other => Err(Yaml::Array(Vec::from(other))),
                    }
                }
                [Yaml::Array(..)] => match <[Yaml; 1]>::try_from(yaml).map_err(Yaml::Array)? {
                    [Yaml::Array(array)] => {
                        let types = match split_array(array) {
                            Ok(types) => types,
                            Err(array) => return Err(Yaml::Array([Yaml::Array(array)].into())),
                        };
                        Ok(Self {
                            doc: None,
                            ty: SimpleTy::Tuple(types),
                        })
                    }
                    other => Err(Yaml::Array(Vec::from(other))),
                },
                _ => Err(Yaml::Array(yaml)),
            },
            other => Err(other),
        }
    }
}
