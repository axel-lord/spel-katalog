use ::std::{io::Cursor, path::Path, rc::Rc};

use ::image::{DynamicImage, ImageFormat::Png};
use ::mlua::{Lua, Table, UserDataMethods};
use ::once_cell::unsync::OnceCell;
use ::rusqlite::{Connection, OptionalExtension, params};

use crate::lua_api::to_runtime;

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

pub fn register_image(
    lua: &Lua,
    conn: Rc<OnceCell<Connection>>,
    db_path: Rc<Path>,
    module: &Table,
) -> ::mlua::Result<()> {
    {
        let db_path = db_path.clone();
        let conn = conn.clone();
        let load_cover =
            lua.create_function(move |lua, slug| lua_load_cover(lua, slug, &db_path, &conn))?;

        module.set("loadCover", load_cover)?;
    }

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
