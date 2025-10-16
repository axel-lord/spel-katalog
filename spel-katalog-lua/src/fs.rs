//! Filesystem functions of lua api.

use ::std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::Path};

use ::image::{DynamicImage, ImageReader};
use ::mlua::{IntoLua, Lua, Table};
use ::yaml_rust2::{Yaml, YamlLoader};

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

fn lua_load_image(lua: &Lua, path: ::mlua::String) -> ::mlua::Result<::mlua::Value> {
    fn ld(path: &Path) -> Result<DynamicImage, ()> {
        ImageReader::open(path)
            .map_err(|err| ::log::error!("could not open image {path:?}\n{err}"))?
            .decode()
            .map_err(|err| ::log::error!("could not decode image {path:?}\n{err}"))
    }
    ld(Path::new(OsStr::from_bytes(&path.as_bytes())))
        .ok()
        .map_or_else(
            || Ok(::mlua::Value::NULL),
            |img| lua.create_any_userdata(img).map(::mlua::Value::UserData),
        )
}

fn lua_load_file(lua: &Lua, path: ::mlua::String) -> ::mlua::Result<::mlua::String> {
    let path = path.as_bytes();
    let path = OsStr::from_bytes(&path);

    let content = ::std::fs::read(path)?;
    lua.create_string(content)
}

fn lua_save_file(
    _lua: &Lua,
    (path, content): (::mlua::String, ::mlua::String),
) -> ::mlua::Result<()> {
    let path = path.as_bytes();
    let path = OsStr::from_bytes(&path);
    let content = content.as_bytes();

    ::std::fs::write(path, content)?;

    Ok(())
}

pub fn register(lua: &Lua, module: &Table) -> ::mlua::Result<()> {
    module.set("loadFile", lua.create_function(lua_load_file)?)?;
    module.set("saveFile", lua.create_function(lua_save_file)?)?;
    module.set("loadYaml", lua.create_function(lua_load_yaml)?)?;
    module.set("loadImage", lua.create_function(lua_load_image)?)?;
    Ok(())
}
