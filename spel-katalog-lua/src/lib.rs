//! Lua api in use by project.

use ::std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::Path, rc::Rc};

use ::mlua::{Lua, Table, Variadic};
use ::once_cell::unsync::OnceCell;
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};

mod cmd;
mod color;
mod fs;
mod image;
mod print;

/// Module skeleton, used to access objects.
#[derive(Debug, Clone)]
struct Skeleton {
    /// Module table.
    pub module: Table,
    /// Color class table.
    pub color: Table,
}

impl Skeleton {
    pub fn new(lua: &Lua, module: Table) -> ::mlua::Result<Self> {
        Ok(Self {
            module,
            color: lua.create_table()?,
        })
    }
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
    let skeleton = Skeleton::new(lua, module)?;

    color::register(&lua, &skeleton)?;
    image::register(&lua, conn, thumb_db_path, &skeleton)?;
    cmd::register(&lua, &skeleton, &sink_builder)?;

    let Skeleton { module, .. } = skeleton;

    fs::register(&lua, &module)?;
    print::register(&lua, &module, &sink_builder)?;

    module.set("getEnv", lua.create_function(lua_get_env)?)?;
    module.set("shellSplit", lua.create_function(lua_shell_split)?)?;
    module.set("pathExists", lua.create_function(lua_path_exists)?)?;
    module.set("None", ::mlua::Value::NULL)?;

    lua.register_module("@spel-katalog", module)?;

    Ok(())
}
