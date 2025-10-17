use ::mlua::{FromLua, Lua, Table, Value};

use crate::{Skeleton, class_instance, init_table};

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

        init_table! {
            initial:
                r = self.r,
                g = self.g,
                b = self.b,
                a = self.a as f64 / 255.0,
        }?;

        class_instance(class, initial)
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

pub fn register(_lua: &Lua, skeleton: &Skeleton) -> ::mlua::Result<()> {
    let color = &skeleton.color;

    init_table! {
        color:
            r = 0,
            g = 0,
            b = 0,
            a = 1.0,
    }?;

    skeleton.module.set("Color", color)?;

    Ok(())
}
