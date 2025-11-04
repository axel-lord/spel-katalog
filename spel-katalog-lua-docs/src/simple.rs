use ::iced::{
    Element,
    widget::{self, text::Span},
};
use ::yaml_rust2::Yaml;

use crate::{Attr, Message, SpanExt, empty_spans};

#[derive(Debug, Clone)]
pub struct Simple<S> {
    pub doc: Option<S>,
    pub ty: S,
    pub attr: Attr,
}

impl<S: AsRef<str>> Simple<S> {
    fn view_<'a>(&'a self, name: Option<&'a str>) -> Element<'a, Message> {
        let Self { doc, ty, attr } = self;
        let ty = Span::new(ty.as_ref()).ty();
        let attr = Span::new(match attr {
            Attr::None => "",
            Attr::Optional => "?",
            Attr::Variadic => "...",
        });
        let [name, sep] = if let Some(name) = name {
            [Span::new(name).name(), Span::new(": ")]
        } else {
            empty_spans()
        };
        let [doc_sep, doc] = if let Some(doc) = doc {
            [Span::new(" # "), Span::new(doc.as_ref())].map(SpanExt::doc)
        } else {
            empty_spans()
        };
        widget::rich_text([name, sep, ty, attr, doc_sep, doc]).into()
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
        match value {
            Yaml::String(ty) => {
                let (ty, attr) = Attr::split_ty(ty);
                Ok(Self {
                    doc: None,
                    ty,
                    attr,
                })
            }
            Yaml::Array(yaml) => match yaml.as_slice() {
                [Yaml::String(..), Yaml::String(..)] => {
                    match <[Yaml; 2]>::try_from(yaml).map_err(Yaml::Array)? {
                        [Yaml::String(ty), Yaml::String(doc)] => {
                            let (ty, attr) = Attr::split_ty(ty);
                            let doc = Some(doc);
                            Ok(Self { ty, doc, attr })
                        }
                        other => Err(Yaml::Array(Vec::from(other))),
                    }
                }
                _ => Err(Yaml::Array(yaml)),
            },
            other => Err(other),
        }
    }
}
