//! Lua api in use by project.

use ::std::{path::Path, rc::Rc};

use ::mlua::{Lua, Table, Variadic};
use ::once_cell::unsync::OnceCell;
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};

mod cmd;
mod color;
mod dialog;
mod fs;
mod image;
mod lua_result;
mod misc;
mod print;
mod yaml;

/// Set values for a table.
/// The result is an ::mlua::Result which should be handled.
///
/// ```
/// let lua = ::mlua::Lua::new().unwrap();
/// let table = lua.create_table().unwrap();
/// init_table! {
///     table:
///         a = 53,
///         b = Some(5.3),
///         c = ::mlua::Value::NULL,
/// }.unwrap()
/// ```
macro_rules! init_table {
    ($tbl:ident: $( $id:ident = $val:expr ),+ $(,)?) => {(|| {
        $( $tbl.set(stringify!($id), $val)?; )*
        Ok::<_, ::mlua::Error>(())
    })()};
}
pub(crate) use init_table;

/// A boxed function which creates and waits on dialogs.
pub type DialogOpener = dyn Fn(String, Vec<String>) -> ::mlua::Result<Option<String>>;

/// Module skeleton, used to access objects.
#[derive(Debug, Clone)]
struct Skeleton {
    /// Module table.
    pub module: Table,
    /// Color class table.
    pub color: Table,
    /// Rect class table.
    pub rect: Table,
}

impl Skeleton {
    pub fn new(lua: &Lua, module: Table) -> ::mlua::Result<Self> {
        Ok(Self {
            module,
            color: create_class(lua)?,
            rect: create_class(lua)?,
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

/// Functionality caller needs to provide.
pub trait Virtual {
    /// Create an object which may request dialogs to be opened. In which case the object blocks
    /// until a choice is made.
    fn dialog_opener(&mut self) -> Box<DialogOpener>;
}

/// Register `@spel-katalog` module with lua interpreter.
pub fn register_module(
    lua: &Lua,
    thumb_db_path: &Path,
    sink_builder: &SinkBuilder,
    module: Option<Table>,
    mut vt: Box<dyn Send + Virtual>,
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
    dialog::register(&lua, &skeleton, vt.dialog_opener())?;

    let Skeleton { module, .. } = skeleton;

    fs::register(&lua, &module)?;
    print::register(&lua, &module, &sink_builder)?;

    module.set("None", ::mlua::Value::NULL)?;

    lua.register_module("@spel-katalog", module)?;

    Ok(())
}
