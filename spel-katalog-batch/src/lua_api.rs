use ::mlua::{IntoLua, Lua};
use ::serde::Serialize;
use ::yaml_rust2::{Yaml, YamlLoader};

use crate::BatchInfo;

fn lua_load_yaml(lua: &Lua, path: String) -> ::mlua::Result<::mlua::Value> {
    let yml = YamlLoader::load_from_str(
        &::std::fs::read_to_string(&path).map_err(::mlua::Error::runtime)?,
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

fn lua_dbg(_: &Lua, mv: ::mlua::MultiValue) -> ::mlua::Result<::mlua::MultiValue> {
    for value in &mv {
        eprintln!("{value:#?}");
    }
    Ok(mv)
}

pub fn lua_batch(
    data: Vec<BatchInfo>,
    script: String,
    settings: ::spel_katalog_settings::Generic,
) -> ::mlua::Result<()> {
    let lua = Lua::new();
    let ser = || ::mlua::serde::Serializer::new(&lua);
    let data = data.serialize(ser())?;
    let settings = settings.serialize(ser())?;

    lua.globals().set("data", data)?;
    lua.globals().set("settings", settings)?;
    lua.globals()
        .set("loadYaml", lua.create_function(lua_load_yaml)?)?;
    lua.globals().set("dbg", lua.create_function(lua_dbg)?)?;
    lua.globals().set("None", ::mlua::Value::NULL)?;
    lua.load(script).exec()?;

    Ok(())
}
