use ::iced::{
    Color, Element,
    widget::{self, text::Span},
};
use ::yaml_rust2::Yaml;

use crate::{Attr, Message};

#[derive(Debug, Clone)]
pub struct Simple {
    pub doc: Option<String>,
    pub ty: String,
    pub attr: Attr,
}

impl Simple {
    pub fn view_anon(&self) -> Element<'_, Message> {
        let Self { doc, ty, attr } = self;
        let ty = Span::new(ty).color(Color::new(0.5, 1.0, 0.5, 1.0));
        let attr = Span::new(match attr {
            Attr::None => "",
            Attr::Optional => "?",
            Attr::Variadic => "...",
        });
        if let Some(doc) = doc {
            let doc = Span::new(doc);
            let doc_sep = Span::new(" ");
            widget::rich_text([ty, attr, doc_sep, doc]).into()
        } else {
            widget::rich_text([ty, attr]).into()
        }
    }

    pub fn view<'a>(&'a self, name: &'a str) -> Element<'a, Message> {
        let Self { doc, ty, attr } = self;
        let ty = Span::new(ty).color(Color::new(0.5, 1.0, 0.5, 1.0));
        let name = Span::new(name);
        let attr = Span::new(match attr {
            Attr::None => "",
            Attr::Optional => "?",
            Attr::Variadic => "...",
        });
        let sep = Span::new(": ");
        if let Some(doc) = doc {
            let doc = Span::new(doc);
            let doc_sep = Span::new(" ");
            widget::rich_text([name, sep, ty, attr, doc_sep, doc]).into()
        } else {
            widget::rich_text([name, sep, ty, attr]).into()
        }
    }
}

impl TryFrom<Yaml> for Simple {
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
