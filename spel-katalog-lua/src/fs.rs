//! Filesystem functions of lua api.

use ::std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::Path};

use ::mlua::{Lua, Table};

fn path_exists(_lua: &Lua, path: ::mlua::String) -> ::mlua::Result<bool> {
    let path = path.as_bytes();
    let path = Path::new(OsStr::from_bytes(&path));
    Ok(path.exists())
}

fn load_file(lua: &Lua, path: ::mlua::String) -> ::mlua::Result<::mlua::String> {
    let path = path.as_bytes();
    let path = OsStr::from_bytes(&path);

    let content = ::std::fs::read(path)?;
    lua.create_string(content)
}

fn save_file(_lua: &Lua, (path, content): (::mlua::String, ::mlua::String)) -> ::mlua::Result<()> {
    let path = path.as_bytes();
    let path = OsStr::from_bytes(&path);
    let content = content.as_bytes();

    ::std::fs::write(path, content)?;

    Ok(())
}

pub fn register(lua: &Lua, module: &Table) -> ::mlua::Result<()> {
    module.set("loadFile", lua.create_function(load_file)?)?;
    module.set("saveFile", lua.create_function(save_file)?)?;
    module.set("pathExists", lua.create_function(path_exists)?)?;
    Ok(())
}
