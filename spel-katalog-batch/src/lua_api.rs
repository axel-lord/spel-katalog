use ::std::{
    ffi::OsStr,
    fmt::Display,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    rc::Rc,
};

use ::mlua::Lua;
use ::once_cell::unsync::OnceCell;
use ::serde::Serialize;
use ::spel_katalog_terminal::{SinkBuilder, SinkIdentity};

use crate::BatchInfo;

mod fs;
mod image;
mod print;
mod cmd;

fn to_runtime<D: Display>(d: D) -> ::mlua::Error {
    ::mlua::Error::runtime(d)
}

fn lua_get_env(lua: &Lua, name: ::mlua::String) -> ::mlua::Result<Option<::mlua::String>> {
    ::std::env::var_os(OsStr::from_bytes(&name.as_bytes()))
        .map(|value| lua.create_string(value.as_bytes()))
        .transpose()
}

pub fn lua_batch(
    data: Vec<BatchInfo>,
    script: String,
    settings: ::spel_katalog_settings::Generic,
    thumb_db_path: PathBuf,
    sink_builder: &SinkBuilder,
) -> ::mlua::Result<()> {
    let lua = Lua::new();
    let ser = || ::mlua::serde::Serializer::new(&lua);
    let data = data.serialize(ser())?;
    let settings = settings.serialize(ser())?;
    let sink_builder =
        sink_builder.with_locked_channel(|| SinkIdentity::StaticName("Lua Batch Script"))?;

    let globals = lua.globals();
    globals.set("data", data)?;
    globals.set("None", ::mlua::Value::NULL)?;

    let conn = Rc::new(OnceCell::new());
    let thumb_db_path = Rc::<Path>::from(thumb_db_path);

    let module = lua.create_table()?;

    image::register_image(&lua, conn, thumb_db_path, &module)?;
    fs::register_fs(&lua, &module)?;
    print::register_print(&lua, &module, &sink_builder)?;
    cmd::register_cmd(&lua, &module, &sink_builder)?;

    module.set("settings", settings)?;
    module.set("getEnv", lua.create_function(lua_get_env)?)?;

    lua.register_module("@spel-katalog", module)?;

    lua.load(script).exec()?;

    Ok(())
}
