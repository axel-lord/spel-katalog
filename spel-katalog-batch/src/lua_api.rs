use ::std::{
    ffi::OsStr,
    fmt::Display,
    io::{Cursor, PipeWriter, Write},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    rc::Rc,
};

use ::image::{DynamicImage, ImageFormat::Png, ImageReader};
use ::mlua::{IntoLua, Lua, UserDataMethods};
use ::once_cell::unsync::OnceCell;
use ::rusqlite::{Connection, OptionalExtension, params};
use ::serde::Serialize;
use ::spel_katalog_terminal::{SinkBuilder, SinkIdentity};
use ::yaml_rust2::{Yaml, YamlLoader};

use crate::BatchInfo;

fn to_runtime<D: Display>(d: D) -> ::mlua::Error {
    ::mlua::Error::runtime(d)
}

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

fn lua_print(_: &Lua, value: String, w: Option<&mut PipeWriter>) -> ::mlua::Result<()> {
    if let Some(w) = w {
        writeln!(w, "{value}")?;
    } else {
        println!("{value}");
    }
    ::log::info!("printed {value}");
    Ok(())
}

fn lua_dbg(
    _: &Lua,
    mv: ::mlua::MultiValue,
    w: Option<&mut PipeWriter>,
) -> ::mlua::Result<::mlua::MultiValue> {
    if let Some(w) = w {
        for value in &mv {
            writeln!(w, "{value:#?}")?;
        }
    } else {
        for value in &mv {
            println!("{value:#?}");
        }
    }
    Ok(mv)
}

fn get_conn<'c>(
    conn: &'c OnceCell<::rusqlite::Connection>,
    db_path: &Path,
) -> ::mlua::Result<&'c ::rusqlite::Connection> {
    conn.get_or_try_init(|| ::rusqlite::Connection::open(db_path))
        .map_err(::mlua::Error::runtime)
}

fn lua_load_cover(
    lua: &Lua,
    slug: String,
    db_path: &Path,
    conn: &OnceCell<::rusqlite::Connection>,
) -> ::mlua::Result<::mlua::Value> {
    get_conn(conn, db_path)?
        .prepare_cached(r"SELECT image FROM images WHERE slug = ?1")
        .map_err(to_runtime)?
        .query_one(params![slug], |row| row.get::<_, Vec<u8>>(0))
        .optional()
        .map_err(to_runtime)?
        .map(|image| ::image::load_from_memory_with_format(&image, Png))
        .transpose()
        .map_err(to_runtime)?
        .map(|image| lua.create_any_userdata(image))
        .transpose()
        .map(|data| data.map_or_else(|| ::mlua::Value::NULL, ::mlua::Value::UserData))
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

fn lua_get_env(lua: &Lua, name: ::mlua::String) -> ::mlua::Result<Option<::mlua::String>> {
    ::std::env::var_os(OsStr::from_bytes(&name.as_bytes()))
        .map(|value| lua.create_string(value.as_bytes()))
        .transpose()
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

pub fn register_image(
    lua: &Lua,
    conn: Rc<OnceCell<Connection>>,
    db_path: Rc<Path>,
) -> ::mlua::Result<()> {
    lua.register_userdata_type::<DynamicImage>(move |r| {
        r.add_method("w", |_, this, _: ()| Ok(this.width()));
        r.add_method("h", |_, this, _: ()| Ok(this.height()));
        r.add_method("save", |_, this, path: String| {
            this.save(&path).map_err(to_runtime)
        });
        r.add_method("saveCover", move |_, this, slug: String| {
            let conn = get_conn(&conn, &db_path)?;

            let mut stmt = conn
                .prepare_cached(r"INSERT INTO images (slug, image) VALUES (?1, ?2)")
                .map_err(to_runtime)?;

            let mut buf = Vec::<u8>::new();
            this.write_to(&mut Cursor::new(&mut buf), Png)
                .map_err(to_runtime)?;

            stmt.execute(params![slug, buf]).map_err(to_runtime)?;

            Ok(())
        });
    })?;
    Ok(())
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

    let conn = Rc::new(OnceCell::new());
    let thumb_db_path = Rc::<Path>::from(thumb_db_path);

    register_image(&lua, conn.clone(), thumb_db_path.clone())?;

    let globals = lua.globals();
    globals.set("data", data)?;
    globals.set("None", ::mlua::Value::NULL)?;

    let [mut stdout, mut stderr] = if let Some([stdout, stderr]) =
        sink_builder.get_pipe_writer_double(|| SinkIdentity::StaticName("Lua Batch"))?
    {
        [Some(stdout), Some(stderr)]
    } else {
        [None, None]
    };

    let load_cover =
        lua.create_function(move |lua, slug| lua_load_cover(lua, slug, &thumb_db_path, &conn))?;
    let dbg = lua.create_function_mut(move |lua, value| lua_dbg(lua, value, stderr.as_mut()))?;
    let prnt = lua.create_function_mut(move |lua, value| lua_print(lua, value, stdout.as_mut()))?;

    let module = lua.create_table()?;
    module.set("settings", settings)?;

    module.set("loadYaml", lua.create_function(lua_load_yaml)?)?;
    module.set("dbg", dbg)?;
    module.set("print", prnt)?;
    module.set("loadCover", load_cover)?;
    module.set("loadImage", lua.create_function(lua_load_image)?)?;
    module.set("getEnv", lua.create_function(lua_get_env)?)?;
    module.set("loadFile", lua.create_function(lua_load_file)?)?;
    module.set("saveFile", lua.create_function(lua_save_file)?)?;

    lua.register_module("@spel-katalog", module)?;

    lua.load(script).exec()?;

    Ok(())
}
