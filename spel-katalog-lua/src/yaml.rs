use ::std::{ffi::OsStr, os::unix::ffi::OsStrExt};

use ::mlua::{BString, IntoLua, Lua};
use ::yaml_rust2::{Yaml, YamlLoader};

use crate::Skeleton;

pub fn load_yaml(lua: &Lua, path: BString) -> ::mlua::Result<::mlua::Value> {
    let yml = YamlLoader::load_from_str(
        &::std::fs::read_to_string(OsStr::from_bytes(&path)).map_err(::mlua::Error::runtime)?,
    )
    .map_err(::mlua::Error::runtime)?
    .into_iter()
    .next()
    .ok_or_else(|| ::mlua::Error::runtime("no yaml was loaded"))?;

    fn conv(lua: &Lua, yml: Yaml) -> ::mlua::Result<::mlua::Value> {
        match yml {
            Yaml::Real(r) => r
                .parse::<f64>()
                .map_err(::mlua::Error::runtime)?
                .into_lua(lua),
            Yaml::Integer(i) => i.into_lua(lua),
            Yaml::String(s) => s.into_lua(lua),
            Yaml::Boolean(b) => b.into_lua(lua),
            Yaml::Array(vec) => vec
                .into_iter()
                .try_fold(lua.create_table()?, |table, yaml| {
                    table.push(conv(lua, yaml)?)?;
                    Ok(table)
                })
                .map(::mlua::Value::Table),
            Yaml::Hash(hash_map) => hash_map
                .into_iter()
                .try_fold(lua.create_table()?, |table, (key, value)| {
                    let key = conv(lua, key)?;
                    let value = conv(lua, value)?;
                    table.set(key, value)?;
                    Ok(table)
                })
                .map(::mlua::Value::Table),
            Yaml::Null | Yaml::BadValue | Yaml::Alias(_) => Ok(::mlua::Value::NULL),
        }
    }

    conv(lua, yml)
}

pub fn register(lua: &Lua, skeleton: &Skeleton) -> ::mlua::Result<()> {
    skeleton
        .module
        .set("loadYaml", lua.create_function(load_yaml)?)?;
    Ok(())
}
