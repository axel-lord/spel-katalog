use ::std::{
    fmt::Display,
    io::Cursor,
    path::{Path, PathBuf},
    rc::Rc,
};

use ::image::{DynamicImage, ImageFormat::Png, ImageReader};
use ::mlua::{IntoLua, Lua, UserDataMethods};
use ::once_cell::unsync::OnceCell;
use ::rusqlite::{OptionalExtension, params};
use ::serde::Serialize;
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

fn lua_dbg(_: &Lua, mv: ::mlua::MultiValue) -> ::mlua::Result<::mlua::MultiValue> {
    for value in &mv {
        eprintln!("{value:#?}");
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

fn lua_load_image(lua: &Lua, path: String) -> ::mlua::Result<::mlua::Value> {
    fn ld(path: &str) -> Result<DynamicImage, ()> {
        ImageReader::open(path)
            .map_err(|err| ::log::error!("could not open image {path:?}\n{err}"))?
            .decode()
            .map_err(|err| ::log::error!("could not decode image {path:?}\n{err}"))
    }
    ld(&path).ok().map_or_else(
        || Ok(::mlua::Value::NULL),
        |img| lua.create_any_userdata(img).map(::mlua::Value::UserData),
    )
}

pub fn lua_batch(
    data: Vec<BatchInfo>,
    script: String,
    settings: ::spel_katalog_settings::Generic,
    thumb_db_path: PathBuf,
) -> ::mlua::Result<()> {
    let lua = Lua::new();
    let ser = || ::mlua::serde::Serializer::new(&lua);
    let data = data.serialize(ser())?;
    let settings = settings.serialize(ser())?;

    let conn = Rc::new(OnceCell::new());
    let thumb_db_path = Rc::<Path>::from(thumb_db_path);

    {
        let conn = conn.clone();
        let db_path = thumb_db_path.clone();
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
    }

    let globals = lua.globals();

    let load_cover =
        lua.create_function(move |lua, slug| lua_load_cover(lua, slug, &thumb_db_path, &conn))?;
    let dbg = lua.create_function(lua_dbg)?;
    let load_yaml = lua.create_function(lua_load_yaml)?;
    let load_image = lua.create_function(lua_load_image)?;

    globals.set("data", data)?;
    globals.set("settings", settings)?;
    globals.set("None", ::mlua::Value::NULL)?;

    globals.set("loadYaml", load_yaml)?;
    globals.set("dbg", dbg)?;
    globals.set("loadCover", load_cover)?;
    globals.set("loadImage", load_image)?;

    lua.load(script).exec()?;

    Ok(())
}
