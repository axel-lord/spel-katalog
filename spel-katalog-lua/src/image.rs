use ::std::{ffi::OsStr, io::Cursor, os::unix::ffi::OsStrExt, path::Path, rc::Rc};

use ::image::{DynamicImage, GenericImage, GenericImageView, ImageFormat::Png, ImageReader, Pixel};
use ::mlua::{FromLua, IntoLua, Lua, Table, UserDataMethods, Value};
use ::nalgebra::Vector3;
use ::once_cell::unsync::OnceCell;
use ::rayon::prelude::*;
use ::rusqlite::{Connection, OptionalExtension, params};

use crate::{Skeleton, color};

fn get_conn<'c>(
    conn: &'c OnceCell<::rusqlite::Connection>,
    db_path: &Path,
) -> ::mlua::Result<&'c ::rusqlite::Connection> {
    conn.get_or_try_init(|| ::rusqlite::Connection::open(db_path))
        .map_err(::mlua::Error::runtime)
}

/// Letterbox params
#[derive(Debug, Clone, Copy)]
struct Letterbox {
    ratio: f64,
    color: color::Color,
}

impl FromLua for Letterbox {
    fn from_lua(value: mlua::Value, lua: &Lua) -> mlua::Result<Self> {
        let default_color = || color::Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        };
        match value {
            Value::Nil => Ok(Letterbox {
                ratio: 1.0,
                color: default_color(),
            }),
            Value::Integer(i) => Ok(Letterbox {
                ratio: i as f64,
                color: default_color(),
            }),
            Value::Number(ratio) => Ok(Letterbox {
                ratio,
                color: default_color(),
            }),
            Value::Table(table) => {
                if let Ok(color) = color::Color::from_lua(Value::Table(table.clone()), lua) {
                    Ok(Letterbox { ratio: 1.0, color })
                } else {
                    let ratio = table.get("ratio").unwrap_or(1.0);
                    let color = table.get("color").unwrap_or_else(|_| default_color());

                    Ok(Letterbox { ratio, color })
                }
            }
            value => Err(::mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Letterbox".to_owned(),
                message: Some("expected nil, Color, table or number != 0".to_owned()),
            }),
        }
    }
}

#[derive(Debug, Clone, FromLua)]
struct Image(DynamicImage);

impl IntoLua for Image {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        lua.create_any_userdata(self).map(Value::UserData)
    }
}

impl Image {
    fn new(width: u32, height: u32) -> ::mlua::Result<Self> {
        if width == 0 || height == 0 {
            return Err(::mlua::Error::RuntimeError(format!(
                "refusing to create an image with dimensions (w: {width}, h: {height})"
            )));
        }
        let img = ::image::DynamicImage::new_rgba8(width, height);
        Ok(Self(img))
    }

    fn letterbox(&self, letterbox: Letterbox) -> ::mlua::Result<Self> {
        let Letterbox { ratio, color } = letterbox;
        let this = &self.0;

        let (w, h) = if ratio < 1.0 {
            let w = f64::from(this.width());
            (
                this.width(),
                (w / ratio).clamp(0.0, f64::from(u32::MAX)) as u32,
            )
        } else {
            let h = f64::from(this.height());
            (
                (h * ratio).clamp(0.0, f64::from(u32::MAX)) as u32,
                this.height(),
            )
        };

        let x_off = (w - this.width()) / 2;
        let y_off = (h - this.height()) / 2;

        let color::Color { r, g, b, a } = color;
        let bg_px = ::image::Rgba([r, g, b, a]);

        let mut outimg = ::image::RgbaImage::new(w, h);
        outimg
            .copy_from(this, x_off, y_off)
            .map_err(::mlua::Error::external)?;

        outimg.par_pixels_mut().for_each(|px| {
            let mut bg_px = bg_px;
            bg_px.blend(px);
            *px = bg_px;
        });

        Ok(Self(DynamicImage::from(outimg)))
    }

    fn load_cover(
        lua: &Lua,
        slug: String,
        db_path: &Path,
        conn: &OnceCell<::rusqlite::Connection>,
    ) -> ::mlua::Result<::mlua::Value> {
        let conn = get_conn(conn, db_path)?;
        let buf = conn
            .prepare_cached(r"SELECT image FROM images WHERE slug = ?1")
            .map_err(::mlua::Error::runtime)?
            .query_one(params![slug], |row| row.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(::mlua::Error::runtime)?;

        let Some(buf) = buf else {
            return Ok(::mlua::Value::NULL);
        };

        let image =
            ::image::load_from_memory_with_format(&buf, Png).map_err(::mlua::Error::runtime)?;

        Self(image).into_lua(lua)
    }

    fn load(lua: &Lua, path: ::mlua::String) -> ::mlua::Result<::mlua::Value> {
        #[inline(always)]
        fn ld(path: &Path) -> Result<DynamicImage, ()> {
            ImageReader::open(path)
                .map_err(|err| ::log::error!("could not open image {path:?}\n{err}"))?
                .decode()
                .map_err(|err| ::log::error!("could not decode image {path:?}\n{err}"))
        }
        ld(Path::new(OsStr::from_bytes(&path.as_bytes())))
            .ok()
            .map_or_else(|| Ok(::mlua::Value::NULL), |img| Self(img).into_lua(lua))
    }

    #[inline]
    fn width(&self) -> ::mlua::Result<u32> {
        Ok(self.0.width())
    }

    #[inline]
    fn height(&self) -> ::mlua::Result<u32> {
        Ok(self.0.height())
    }

    fn at(&self, lua: &Lua, color_class: &Table, x: u32, y: u32) -> ::mlua::Result<Table> {
        let this = &self.0;
        if !this.in_bounds(x, y) {
            let w = this.width();
            let h = this.height();
            return Err(::mlua::Error::RuntimeError(format!(
                "point (x: {x}, y: {y}) is outside of bounds (w: {w}, h: {h})"
            )));
        }
        let [r, g, b, a] = this.get_pixel(x, y).0;
        let clr = color::Color { r, g, b, a };
        clr.to_table(lua, &color_class)
    }

    fn set(&mut self, x: u32, y: u32, clr: color::Color) -> ::mlua::Result<()> {
        let this = &mut self.0;
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
    }

    fn avg(&self, lua: &Lua, color_class: &Table) -> ::mlua::Result<Table> {
        let this = &self.0;
        let img;
        let img = if let Some(img) = this.as_rgba8() {
            img
        } else {
            img = this.to_rgba8();
            &img
        };

        let w_factor = 1.0 / f64::from(img.width());
        let h_factor = 1.0 / f64::from(img.height());

        let r = img
            .par_pixels()
            .fold(Vector3::zeros, |avg, px| {
                let [r, g, b, a] = px.0.map(f64::from);
                let a = a / 255.0;
                avg + Vector3::new(r, g, b) * a * w_factor * h_factor
            })
            .reduce(Vector3::zeros, |a, b| a + b)
            .map(|i| i.clamp(0.0, 255.0) as u8);
        let [r, g, b] = *AsRef::<[u8; 3]>::as_ref(&r);

        color::Color { r, g, b, a: 255 }.to_table(lua, color_class)
    }

    fn save(&self, path: ::mlua::String) -> ::mlua::Result<()> {
        self.0
            .save(OsStr::from_bytes(&path.as_bytes()))
            .map_err(::mlua::Error::runtime)
    }

    fn save_cover(
        &self,
        slug: String,
        db_path: &Path,
        conn: &OnceCell<::rusqlite::Connection>,
    ) -> ::mlua::Result<()> {
        let this = &self.0;
        let conn = get_conn(conn, db_path)?;

        let mut stmt = conn
            .prepare_cached(r"INSERT INTO images (slug, image) VALUES (?1, ?2)")
            .map_err(::mlua::Error::runtime)?;

        let mut buf = Vec::<u8>::new();
        this.write_to(&mut Cursor::new(&mut buf), Png)
            .map_err(::mlua::Error::runtime)?;

        stmt.execute(params![slug, buf])
            .map_err(::mlua::Error::runtime)?;

        Ok(())
    }

    fn register(
        lua: &Lua,
        skeleton: &Skeleton,
        conn: Rc<OnceCell<Connection>>,
        db_path: Rc<Path>,
    ) -> ::mlua::Result<()> {
        let module = &skeleton.module;

        let load_cover = lua.create_function({
            let db_path = Rc::clone(&db_path);
            let conn = Rc::clone(&conn);

            move |lua, (_, slug): (Table, _)| Image::load_cover(lua, slug, &db_path, &conn)
        })?;
        let new_image = lua.create_function(|_, (_, w, h): (Table, _, _)| Image::new(w, h))?;
        let load_image =
            lua.create_function(|lua, (_, path): (Table, _)| Image::load(lua, path))?;

        let class = lua.create_table()?;

        class.set("loadCover", load_cover)?;
        class.set("new", new_image)?;
        class.set("load", load_image)?;

        module.set("Image", class)?;

        let color_class = skeleton.color.clone();
        lua.register_userdata_type::<Image>(move |r| {
            r.add_method("w", |_, this, _: ()| this.width());
            r.add_method("h", |_, this, _: ()| this.height());

            let class = color_class.clone();
            r.add_method("at", move |lua, this, (x, y)| this.at(lua, &class, x, y));
            r.add_method_mut("set", |_, this, (x, y, clr)| this.set(x, y, clr));

            r.add_method("letterbox", |_, this, letterbox| this.letterbox(letterbox));

            let class = color_class.clone();
            r.add_method("avg", move |lua, this, _: ()| this.avg(lua, &class));
            r.add_method("save", |_, this, path| this.save(path));
            r.add_method("saveCover", move |_, this, slug| {
                this.save_cover(slug, &db_path, &conn)
            });
        })
    }
}

pub fn register(
    lua: &Lua,
    conn: Rc<OnceCell<Connection>>,
    db_path: Rc<Path>,
    skeleton: &Skeleton,
) -> ::mlua::Result<()> {
    Image::register(lua, skeleton, conn.clone(), db_path.clone())
}
