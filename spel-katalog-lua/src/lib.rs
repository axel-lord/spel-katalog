//! Lua api in use by project.

use ::std::{path::Path, rc::Rc};

use ::mlua::{Lua, Table, Variadic};
use ::once_cell::unsync::OnceCell;
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};

mod cmd;
mod color;
mod fs;
mod image;
mod lua_result;
mod misc;
mod print;
mod yaml;

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
            color: create_class(lua)?,
        })
    }
}

/// Make a table an instance of a class.
#[inline]
fn class_instance(class: &Table, initial: Table) -> ::mlua::Result<Table> {
    initial.set_metatable(Some(class.clone()))?;
    Ok(initial)
}

/// Create a class with `__index` set to self, and a new function.
fn create_class(lua: &Lua) -> ::mlua::Result<Table> {
    let class = lua.create_table()?;
    class.set("__index", &class)?;

    fn new(lua: &Lua, class: Table, tables: Variadic<Table>) -> ::mlua::Result<Variadic<Table>> {
        if tables.is_empty() {
            Ok(Variadic::from_iter([class_instance(
                &class,
                lua.create_table()?,
            )?]))
        } else {
            tables
                .iter()
                .try_for_each(|table| table.set_metatable(Some(class.clone())))?;
            Ok(tables)
        }
    }

    class.set(
        "new",
        lua.create_function(move |lua, (class, tables)| new(lua, class, tables))?,
    )?;

    Ok(class)
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
    misc::register(&lua, &skeleton)?;
    yaml::register(&lua, &skeleton)?;

    let Skeleton { module, .. } = skeleton;

    fs::register(&lua, &module)?;
    print::register(&lua, &module, &sink_builder)?;

    module.set("None", ::mlua::Value::NULL)?;

    lua.register_module("@spel-katalog", module)?;

    Ok(())
}
