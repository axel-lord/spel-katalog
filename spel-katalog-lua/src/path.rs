use ::std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

use ::mlua::{BString, Lua, Variadic};
use ::tap::{Conv, Pipe};

use crate::{Skeleton, make_module};

fn bytes_as_path(bytes: &[u8]) -> &Path {
    Path::new(OsStr::from_bytes(bytes))
}

fn path_buf_from_bstring(bstring: BString) -> PathBuf {
    PathBuf::from(OsString::from_vec(bstring.into()))
}

fn bstring_from_path_buf(path_buf: PathBuf) -> BString {
    path_buf.into_os_string().into_vec().into()
}

pub fn register(lua: &Lua, skeleton: &Skeleton) -> ::mlua::Result<()> {
    let module = lua.create_table()?;

    module.set(
        "exists",
        lua.create_function(|_, path: BString| Ok(bytes_as_path(&path).exists()))?,
    )?;

    module.set(
        "parent",
        lua.create_function(|_, path: BString| {
            let mut path = path_buf_from_bstring(path);
            if !path.pop() {
                return Ok(None);
            }
            Ok(Some(bstring_from_path_buf(path)))
        })?,
    )?;

    module.set(
        "join",
        lua.create_function(|_, paths: Variadic<BString>| {
            paths
                .into_iter()
                .map(path_buf_from_bstring)
                .collect::<PathBuf>()
                .pipe(bstring_from_path_buf)
                .pipe(Ok)
        })?,
    )?;

    module.set(
        "canonicalize",
        lua.create_function(|_, path| {
            path_buf_from_bstring(path)
                .canonicalize()
                .ok()
                .map(bstring_from_path_buf)
                .pipe(Ok)
        })?,
    )?;

    module.set(
        "fileName",
        lua.create_function(|_, path| {
            path_buf_from_bstring(path)
                .file_name()
                .map(|name| BString::from(name.as_bytes()))
                .pipe(Ok)
        })?,
    )?;

    module.set(
        "fileStem",
        lua.create_function(|_, path| {
            path_buf_from_bstring(path)
                .file_stem()
                .map(|stem| BString::from(stem.as_bytes()))
                .pipe(Ok)
        })?,
    )?;

    module.set(
        "extension",
        lua.create_function(|_, path| {
            path_buf_from_bstring(path)
                .extension()
                .map(|ext| BString::from(ext.as_bytes()))
                .pipe(Ok)
        })?,
    )?;

    module.set(
        "split",
        lua.create_function(|_, path| {
            path_buf_from_bstring(path)
                .components()
                .map(|comp| AsRef::<OsStr>::as_ref(&comp).as_bytes().conv::<BString>())
                .collect::<Vec<_>>()
                .pipe(Ok)
        })?,
    )?;

    make_module(lua, &module)?;
    skeleton.module.set("path", module)?;
    Ok(())
}
