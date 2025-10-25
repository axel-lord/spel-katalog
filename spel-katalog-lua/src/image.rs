use ::std::{ffi::OsStr, io::Cursor, os::unix::ffi::OsStrExt, path::Path, rc::Rc};

use ::image::{
    DynamicImage, GenericImage, GenericImageView, ImageFormat::Png, ImageReader, Pixel,
    imageops::FilterType,
};
use ::mlua::{AnyUserData, FromLua, Function, IntoLua, Lua, Table, UserDataMethods, Value};
use ::nalgebra::Vector3;
use ::once_cell::unsync::OnceCell;
use ::rayon::prelude::*;
use ::rusqlite::{Connection, OptionalExtension, params};
use ::tap::Pipe;

use crate::{Skeleton, color, init_table, lua_result::LuaResult, make_class};

fn get_conn<'c>(
    conn: &'c OnceCell<::rusqlite::Connection>,
    db_path: &Path,
) -> ::mlua::Result<&'c ::rusqlite::Connection> {
    conn.get_or_try_init(|| ::rusqlite::Connection::open(db_path))
        .map_err(::mlua::Error::runtime)
}

/// Rectangle consisting of a point, width and height.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl FromLua for Rect {
    fn from_lua(value: Value, _lua: &Lua) -> mlua::Result<Self> {
        value
            .as_table()
            .ok_or_else(|| ::mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Rect".to_owned(),
                message: Some("expected table".to_owned()),
            })
            .and_then(|table| {
                Ok(Self {
                    x: table.get("x")?,
                    y: table.get("y")?,
                    w: table.get("w")?,
                    h: table.get("h")?,
                })
            })
    }
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

/// An image resize filter
#[derive(Debug, Clone, Copy)]
struct Filter(FilterType);

impl FromLua for Filter {
    fn from_lua(value: Value, _lua: &Lua) -> mlua::Result<Self> {
        let Value::String(s) = value else {
            return Err(::mlua::Error::FromLuaConversionError {
                from: value.type_name(),
                to: "Filter".to_owned(),
                message: Some("expected string".to_owned()),
            });
        };

        let lower = s.as_bytes().to_ascii_lowercase();

        let f = match lower.as_slice() {
            b"nearest" => FilterType::Nearest,
            b"triangle" => FilterType::Triangle,
            b"catmullrom" => FilterType::CatmullRom,
            b"gaussian" => FilterType::Gaussian,
            b"lanczos3" => FilterType::Lanczos3,
            other => {
                return Err(::mlua::Error::RuntimeError(format!(
                    "unknown filter {other:?}",
                    other = OsStr::from_bytes(other)
                )));
            }
        };

        Ok(Self(f))
    }
}

#[derive(Debug, Clone, FromLua)]
pub struct Image(DynamicImage);

impl IntoLua for Image {
    fn into_lua(self, lua: &Lua) -> mlua::Result<Value> {
        lua.create_any_userdata(self).map(Value::UserData)
    }
}

impl Image {
    fn new(_c: &::mlua::Table, width: u32, height: u32) -> ::mlua::Result<Self> {
        if width == 0 || height == 0 {
            return Err(::mlua::Error::RuntimeError(format!(
                "refusing to create an image with dimensions (w: {width}, h: {height})"
            )));
        }

        let img = DynamicImage::new_rgba8(width, height);
        Ok(Self(img))
    }

    fn stretch(&self, width: u32, height: u32, filter: Option<Filter>) -> ::mlua::Result<Self> {
        if width == 0 || height == 0 {
            return Err(::mlua::Error::RuntimeError(format!(
                "refusing to stretch an image to dimensions (w: {width}, h: {height})"
            )));
        }

        Ok(Self(self.0.resize_exact(
            width,
            height,
            filter.map_or_else(|| FilterType::Nearest, |Filter(f)| f),
        )))
    }

    fn resize(&self, width: u32, height: u32, filter: Option<Filter>) -> ::mlua::Result<Self> {
        if width == 0 || height == 0 {
            return Err(::mlua::Error::RuntimeError(format!(
                "refusing to resize an image to dimensions (w: {width}, h: {height})"
            )));
        }

        Ok(Self(self.0.resize(
            width,
            height,
            filter.map_or_else(|| FilterType::Nearest, |Filter(f)| f),
        )))
    }

    #[inline]
    fn flip_h(&self) -> Self {
        Self(self.0.fliph())
    }

    #[inline]
    fn flip_v(&self) -> Self {
        Self(self.0.flipv())
    }

    fn overlay(&self, other: AnyUserData, x: Option<u32>, y: Option<u32>) -> ::mlua::Result<Self> {
        let other = other.borrow::<Self>()?;
        let other = &other.0;
        let (width, height) = (other.width(), other.height());
        let (x, y) = (x.unwrap_or(0), y.unwrap_or(0));

        _ = self
            .0
            .try_view(x, y, width, height)
            .map_err(::mlua::Error::runtime)?;

        let mut dest = self.0.to_rgba8();
        dest.sub_image(x, y, width, height)
            .inner_mut()
            .par_enumerate_pixels_mut()
            .for_each(|(x, y, px)| {
                let other = other.get_pixel(x, y);
                px.blend(&other);
            });

        Ok(Self(DynamicImage::from(dest)))
    }

    fn map_color(&self, lua: &Lua, color_class: &Table, map: Function) -> ::mlua::Result<Self> {
        let mut outimg = self.0.to_rgba8();

        outimg.enumerate_pixels_mut().try_for_each(|(x, y, px)| {
            let px_color = color::Color::from(*px).to_table(lua, color_class)?;

            let Some(color) = map.call::<Option<color::Color>>((px_color, x, y))? else {
                return Ok(());
            };

            *px = color.into();

            Ok::<_, ::mlua::Error>(())
        })?;

        Ok(Self(DynamicImage::from(outimg)))
    }

    fn crop(&self, rect: Rect) -> ::mlua::Result<Self> {
        let Rect { x, y, w, h } = rect;
        let this = &self.0;

        let view = this.try_view(x, y, w, h).map_err(::mlua::Error::runtime)?;

        Ok(Self(DynamicImage::from(view.to_image())))
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

    pub fn load_cover(
        slug: String,
        db_path: &Path,
        conn: &OnceCell<::rusqlite::Connection>,
    ) -> ::mlua::Result<LuaResult<Self, String>> {
        let conn = get_conn(conn, db_path)?;
        let buf = conn
            .prepare_cached(r"SELECT image FROM images WHERE slug = ?1")
            .map_err(::mlua::Error::runtime)?
            .query_one(params![slug], |row| row.get::<_, Vec<u8>>(0))
            .optional()
            .map_err(::mlua::Error::runtime)?;

        let Some(buf) = buf else {
            return LuaResult::Err(format!("could nod load cover for slug {slug:?}")).wrap_ok();
        };

        let image = ::image::load_from_memory_with_format(&buf, Png)
            .map_err(::mlua::Error::runtime)?
            .pipe(Self);

        LuaResult::Ok(image).wrap_ok()
    }

    fn load(_c: &::mlua::Table, path: ::mlua::String) -> ::mlua::Result<LuaResult<Self, String>> {
        let image = ImageReader::open(OsStr::from_bytes(&path.as_bytes()))
            .map_err(|err| format!("could not open image {path:?}, {err}"))
            .and_then(|image| {
                image
                    .decode()
                    .map_err(|err| format!("could not decode image {path:?}, {err}"))
            })
            .map(Self);

        LuaResult::from(image).wrap_ok()
    }

    #[inline]
    fn width(&self) -> u32 {
        self.0.width()
    }

    #[inline]
    fn height(&self) -> u32 {
        self.0.height()
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

    pub fn save_cover(
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

            move |_, (_, slug): (::mlua::Table, _)| Image::load_cover(slug, &db_path, &conn)
        })?;
        let new_image = lua.create_function(|_, (c, w, h)| Image::new(&c, w, h))?;
        let load_image = lua.create_function(|_, (c, path)| Image::load(&c, path))?;

        let class = lua.create_table()?;

        class.set("loadCover", load_cover)?;
        class.set("new", new_image)?;
        class.set("load", load_image)?;

        module.set("Image", class)?;

        let color_class = skeleton.color.clone();
        lua.register_userdata_type::<Image>(move |r| {
            r.add_method("w", |_, this, _: ()| Ok(this.width()));
            r.add_method("h", |_, this, _: ()| Ok(this.height()));

            let class = color_class.clone();
            r.add_method("at", move |lua, this, (x, y)| this.at(lua, &class, x, y));

            r.add_method_mut("set", |_, this, (x, y, clr)| this.set(x, y, clr));

            r.add_method("letterbox", |_, this, letterbox| this.letterbox(letterbox));
            r.add_method("crop", |_, this, rect| this.crop(rect));
            r.add_method("flipH", |_, this, _: ()| Ok(this.flip_h()));
            r.add_method("flipV", |_, this, _: ()| Ok(this.flip_v()));
            r.add_method("overlay", |_, this, (other, x, y)| {
                this.overlay(other, x, y)
            });
            r.add_method("stretch", |_, this, (width, height, filter)| {
                this.stretch(width, height, filter)
            });
            r.add_method("resize", |_, this, (width, height, filter)| {
                this.resize(width, height, filter)
            });

            let class = color_class.clone();
            r.add_method("mapColor", move |lua, this, map| {
                this.map_color(lua, &class, map)
            });

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
    let rect = &skeleton.rect;
    make_class(lua, rect)?;
    init_table! {
        rect:
            x = 0,
            y = 0,
            w = 0,
            h = 0,
    }?;
    skeleton.module.set("Rect", rect)?;

    Image::register(lua, skeleton, conn.clone(), db_path.clone())
}
