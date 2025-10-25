use ::std::{ffi::OsStr, os::unix::ffi::OsStrExt, sync::Arc};

use ::mlua::{Lua, LuaSerdeExt, Variadic};
use ::spel_katalog_formats::AdditionalConfig;

use crate::{Skeleton, Virtual};

fn get_env(lua: &Lua, name: ::mlua::String) -> ::mlua::Result<Option<::mlua::String>> {
    ::std::env::var_os(OsStr::from_bytes(&name.as_bytes()))
        .map(|value| lua.create_string(value.as_bytes()))
        .transpose()
}

fn shell_split(_lua: &Lua, args: Variadic<String>) -> ::mlua::Result<Vec<String>> {
    let mut out = Vec::new();
    for arg in args {
        let split = ::shell_words::split(&arg);
        out.extend(split.map_err(|err| ::mlua::Error::external(err))?);
    }
    Ok(out)
}

pub fn set_attr(
    lua: &Lua,
    id: i64,
    attr: String,
    value: String,
    vt: &dyn Virtual,
) -> ::mlua::Result<::mlua::Value> {
    let path = vt.additional_config_path(id)?;
    let mut initial = ::std::fs::read_to_string(&path).map_or_else(
        |err| match err.kind() {
            ::std::io::ErrorKind::NotFound => Ok(AdditionalConfig::default()),
            _ => Err(::mlua::Error::external(err)),
        },
        |content| ::toml::from_str(&content).map_err(::mlua::Error::external),
    )?;

    initial.attrs.insert(attr, value);

    let content = ::toml::to_string_pretty(&initial).map_err(::mlua::Error::external)?;
    let table = lua.to_value(&initial.attrs)?;

    ::std::fs::write(&path, content.as_bytes()).map_err(::mlua::Error::external)?;

    Ok(table)
}

pub fn register(lua: &Lua, skeleton: &Skeleton, vt: Arc<dyn Virtual>) -> ::mlua::Result<()> {
    let module = &skeleton.module;
    module.set("getEnv", lua.create_function(get_env)?)?;
    module.set("shellSplit", lua.create_function(shell_split)?)?;
    module.set(
        "setAttr",
        lua.create_function(move |lua, (id, attr, value)| {
            set_attr(lua, id, attr, value, vt.as_ref())
        })?,
    )?;
    Ok(())
}
