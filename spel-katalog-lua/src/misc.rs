use ::std::{ffi::OsStr, os::unix::ffi::OsStrExt};

use ::mlua::{Lua, Variadic};

use crate::Skeleton;

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

pub fn register(lua: &Lua, skeleton: &Skeleton) -> ::mlua::Result<()> {
    let module = &skeleton.module;
    module.set("getEnv", lua.create_function(get_env)?)?;
    module.set("shellSplit", lua.create_function(shell_split)?)?;
    Ok(())
}
