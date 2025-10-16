//! Lua api in use by project.

use ::std::{path::Path, rc::Rc};

use ::mlua::{Lua, Table};
use ::once_cell::unsync::OnceCell;
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};

mod cmd;
mod color;
mod fs;
mod image;
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
            color: lua.create_table()?,
        })
    }
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
