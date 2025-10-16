//! Lua api entrypoints.

use ::std::{
    ffi::OsStr,
    fmt::Display,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    rc::Rc,
};

use ::mlua::{Lua, Table, Variadic};
use ::once_cell::unsync::OnceCell;
use ::serde::Serialize;
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};

use crate::BatchInfo;

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

fn serializer(lua: &Lua) -> ::mlua::serde::Serializer<'_> {
    ::mlua::serde::Serializer::new(lua)
}

/// Register `@spel-katalog` module with lua interpreter.
pub fn register_spel_katalog(
    lua: &Lua,
    settings: ::spel_katalog_settings::Generic,
    thumb_db_path: PathBuf,
    sink_builder: &SinkBuilder,
    module: Option<Table>,
) -> ::mlua::Result<()> {
    let settings = settings.serialize(serializer(lua))?;
    let sink_builder =
        sink_builder.with_locked_channel(|| SinkIdentity::StaticName("Lua Batch Script"))?;

    let conn = Rc::new(OnceCell::new());
    let thumb_db_path = Rc::<Path>::from(thumb_db_path);

    let module = if let Some(module) = module {
        module
    } else {
        lua.create_table()?
    };

    color::register_color(&lua, &module)?;
    image::register_image(&lua, conn, thumb_db_path, &module)?;
    fs::register_fs(&lua, &module)?;
    print::register_print(&lua, &module, &sink_builder)?;
    cmd::register_cmd(&lua, &module, &sink_builder)?;

    module.set("settings", settings)?;
    module.set("getEnv", lua.create_function(lua_get_env)?)?;
    module.set("shellSplit", lua.create_function(lua_shell_split)?)?;
    module.set("pathExists", lua.create_function(lua_path_exists)?)?;
    module.set("None", ::mlua::Value::NULL)?;

    lua.register_module("@spel-katalog", module)?;

    Ok(())
}

/// Run a lua script with a batch.
pub fn lua_batch(
    data: Vec<BatchInfo>,
    script: String,
    settings: ::spel_katalog_settings::Generic,
    thumb_db_path: PathBuf,
    sink_builder: &SinkBuilder,
) -> ::mlua::Result<()> {
    let lua = Lua::new();
    let data = data.serialize(serializer(&lua))?;
    let module = lua.create_table()?;

    module.set("data", data)?;

    register_spel_katalog(&lua, settings, thumb_db_path, sink_builder, Some(module))?;

    lua.load(script).exec()?;

    Ok(())
}
