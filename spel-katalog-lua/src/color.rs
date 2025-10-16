use ::mlua::{FromLua, Lua, Table, Value, Variadic};

use crate::Skeleton;

/// A color as a rust type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Convert to a table, using the provided class.
    pub fn to_table(&self, lua: &Lua, class: &Table) -> ::mlua::Result<::mlua::Table> {
        let initial = lua.create_table()?;

        initial.set("r", self.r)?;
        initial.set("g", self.g)?;
        initial.set("b", self.b)?;
        initial.set("a", self.a as f64 / 255.0)?;

        new_color(class, initial)
    }
}

impl FromLua for Color {
    fn from_lua(value: mlua::Value, _lua: &Lua) -> mlua::Result<Self> {
        let Value::Table(table) = value else {
            return Err(::mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Color".to_owned(),
                message: Some("expected table".to_owned()),
            });
        };

        let get_f = |name: &str| -> ::mlua::Result<u8> {
            Ok(table.get::<f64>(name)?.clamp(0.0, 255.0) as u8)
        };

        let [r, g, b] = ["r", "g", "b"].map(get_f);
        Ok(Self {
            r: r?,
            b: b?,
            g: g?,
            a: (table.get::<f64>("a")?.clamp(0.0, 1.0) * 255.0) as u8,
        })
    }
}

pub fn new_color(class: &Table, initial: Table) -> ::mlua::Result<Table> {
    initial.set_metatable(Some(class.clone()))?;
    Ok(initial)
}

pub fn register(lua: &Lua, skeleton: &Skeleton) -> ::mlua::Result<()> {
    let color = &skeleton.color;
    color.set("r", 0)?;
    color.set("g", 0)?;
    color.set("b", 0)?;
    color.set("a", 1.0)?;
    color.set("__index", color)?;
    skeleton.module.set("Color", color)?;

    color.set(
        "new",
        lua.create_function(
            move |lua,
                  (class, tables): (Table, Variadic<Table>)|
                  -> ::mlua::Result<Variadic<Table>> {
                if tables.is_empty() {
                    lua.create_table()
                        .and_then(|initial| new_color(&class, initial))
                        .map(|color| Variadic::from_iter([color]))
                } else {
                    tables
                        .into_iter()
                        .map(|initial| new_color(&class, initial))
                        .collect::<Result<Variadic<_>, _>>()
                }
            },
        )?,
    )?;

    Ok(())
}
