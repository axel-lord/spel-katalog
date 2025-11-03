use ::std::sync::LazyLock;

use ::yaml_rust2::Yaml;

use crate::{Item, Map};

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
pub struct Table {
    pub doc: Option<String>,
    pub union: Vec<Item>,
    pub fields: Map<String, Item>,
    pub params: Map<String, Item>,
    pub r#return: Vec<Item>,
    pub r#enum: Vec<(String, Option<String>)>,
}

impl TryFrom<Yaml> for Table {
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
