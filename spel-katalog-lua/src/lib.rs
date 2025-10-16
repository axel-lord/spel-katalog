//! Lua api in use by project.

use ::std::{ffi::OsStr, fmt::Display, os::unix::ffi::OsStrExt, path::Path, rc::Rc};

use ::mlua::{Lua, Table, Variadic};
use ::once_cell::unsync::OnceCell;
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};

mod cmd;
mod color;
mod fs;
mod image;
mod print;

fn to_runtime<D: Display>(d: D) -> ::mlua::Error {
    ::mlua::Error::runtime(d)
}

fn lua_get_env(lua: &Lua, name: ::mlua::String) -> ::mlua::Result<Option<::mlua::String>> {
    ::std::env::var_os(OsStr::from_bytes(&name.as_bytes()))
        .map(|value| lua.create_string(value.as_bytes()))
        .transpose()
}

fn lua_shell_split(_lua: &Lua, args: Variadic<String>) -> ::mlua::Result<Vec<String>> {
    let mut out = Vec::new();
    for arg in args {
        let split = ::shell_words::split(&arg);
        out.extend(split.map_err(|err| ::mlua::Error::external(err))?);
    }
    Ok(out)
}

fn lua_path_exists(_lua: &Lua, path: ::mlua::String) -> ::mlua::Result<bool> {
    let path = path.as_bytes();
    let path = Path::new(OsStr::from_bytes(&path));
    Ok(path.exists())
}

/// Register `@spel-katalog` module with lua interpreter.
pub fn register_module(
    lua: &Lua,
    thumb_db_path: &Path,
    sink_builder: &SinkBuilder,
    module: Option<Table>,
) -> ::mlua::Result<()> {
    let sink_builder =
        sink_builder.with_locked_channel(|| SinkIdentity::StaticName("Lua Script"))?;

    let conn = Rc::new(OnceCell::new());
    let thumb_db_path = Rc::<Path>::from(thumb_db_path);
    let module = module.map(Ok).unwrap_or_else(|| lua.create_table())?;

    color::register_color(&lua, &module)?;
    image::register_image(&lua, conn, thumb_db_path, &module)?;
    fs::register_fs(&lua, &module)?;
    print::register_print(&lua, &module, &sink_builder)?;
    cmd::register_cmd(&lua, &module, &sink_builder)?;

    module.set("getEnv", lua.create_function(lua_get_env)?)?;
    module.set("shellSplit", lua.create_function(lua_shell_split)?)?;
    module.set("pathExists", lua.create_function(lua_path_exists)?)?;
    module.set("None", ::mlua::Value::NULL)?;

    lua.register_module("@spel-katalog", module)?;

    Ok(())
}
