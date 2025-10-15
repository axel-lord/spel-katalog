use ::std::{io::Cursor, path::Path, rc::Rc};

use ::image::{DynamicImage, GenericImage, GenericImageView, ImageFormat::Png};
use ::mlua::{Lua, Table, UserDataMethods};
use ::once_cell::unsync::OnceCell;
use ::rusqlite::{Connection, OptionalExtension, params};

use crate::lua_api::{color, to_runtime};

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
    let color = color::get_class(module)?;

    lua.register_userdata_type::<DynamicImage>(move |r| {
        r.add_method("w", |_, this, _: ()| Ok(this.width()));
        r.add_method("h", |_, this, _: ()| Ok(this.height()));

        let class = color.clone();
        r.add_method("at", move |lua, this, (x, y): (u32, u32)| {
            if !this.in_bounds(x, y) {
                let w = this.width();
                let h = this.height();
                return Err(::mlua::Error::RuntimeError(format!(
                    "point (x: {x}, y: {y}) is outside of bounds (w: {w}, h: {h})"
                )));
            }
            let [r, g, b, a] = this.get_pixel(x, y).0;
            let clr = color::Color { r, g, b, a };
            clr.to_table(lua, &class)
        });
        r.add_method_mut("set", |_, this, (x, y, clr): (u32, u32, color::Color)| {
            if !this.in_bounds(x, y) {
                let w = this.width();
                let h = this.height();
                return Err(::mlua::Error::RuntimeError(format!(
                    "point (x: {x}, y: {y}) is outside of bounds (w: {w}, h: {h})"
                )));
            }
            let color::Color { r, g, b, a } = clr;
            this.put_pixel(x, y, ::image::Rgba([r, g, b, a]));
            Ok(())
        });
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
