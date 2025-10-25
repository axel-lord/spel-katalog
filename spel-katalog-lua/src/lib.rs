//! Lua api in use by project.

use ::std::{
    collections::HashMap,
    fmt::Debug,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
};

use ::mlua::{Lua, LuaSerdeExt, Table, Variadic};
use ::once_cell::unsync::OnceCell;
use ::rustc_hash::FxHashMap;
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};

mod cmd;
mod color;
mod dialog;
mod fs;
mod game_data;
mod image;
mod lua_result;
mod misc;
mod path;
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

/// Module skeleton, used to access objects.
#[derive(Debug, Clone)]
pub struct Skeleton {
    /// Module table.
    pub module: Table,
    /// Color class table.
    pub color: Table,
    /// Rect class table.
    pub rect: Table,
    /// GameData class table.
    pub game_data: Table,
}

impl Skeleton {
    fn new(lua: &Lua, module: Table) -> ::mlua::Result<Self> {
        Ok(Self {
            module,
            color: lua.create_table()?,
            rect: lua.create_table()?,
            game_data: lua.create_table()?,
        })
    }
}

/// Make a table an instance of a class.
#[inline]
fn class_instance(class: &Table, initial: Table) -> ::mlua::Result<Table> {
    initial.set_metatable(Some(class.clone()))?;
    Ok(initial)
}

/// Set the class of a table.
pub fn set_class(tbl: &Table, class: &Table) -> ::mlua::Result<()> {
    tbl.set_metatable(Some(class.clone()))
}

/// Make the given table into a class with `__index` set to self, and a new function.
fn make_class(lua: &Lua, class: &Table) -> ::mlua::Result<()> {
    class.set("__index", class)?;

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
    Ok(())
}

/// Functionality caller needs to provide.
pub trait Virtual: 'static + Debug + Send + Sync {
    /// Open a dialog window with the given text and buttons.
    fn open_dialog(&self, text: String, buttons: Vec<String>) -> ::mlua::Result<Option<String>>;

    /// Create a dictionary of available lua modules and their source code.
    fn available_modules(&self) -> FxHashMap<String, String>;

    /// Get path to thumbnail cache db.
    fn thumb_db_path(&self) -> ::mlua::Result<PathBuf>;

    /// Get path to additional config dir for a game.
    fn additional_config_path(&self, game_id: i64) -> ::mlua::Result<PathBuf>;

    /// Get settings as a hash map.
    fn settings(&self) -> ::mlua::Result<HashMap<&'_ str, String>>;
}

/// Module info used for registration,
#[derive(Debug, Clone)]
pub struct Module<'dep> {
    /// Sink builder.
    pub sink_builder: &'dep SinkBuilder,
    /// Virtual table to use for external functions.
    pub vt: Arc<dyn Virtual>,
}

impl Module<'_> {
    /// Register module to lua instance.
    pub fn register(self, lua: &Lua) -> ::mlua::Result<Skeleton> {
        let Self { sink_builder, vt } = self;
        register_module(lua, Rc::from(vt.thumb_db_path()?), sink_builder, vt)
    }
}

/// Register `spel-katalog` module with lua interpreter.
fn register_module(
    lua: &Lua,
    thumb_db_path: Rc<Path>,
    sink_builder: &SinkBuilder,
    vt: Arc<dyn Virtual>,
) -> ::mlua::Result<Skeleton> {
    let sink_builder =
        sink_builder.with_locked_channel(|| SinkIdentity::StaticName("Lua Script"))?;

    let conn = Rc::new(OnceCell::new());
    let skeleton = Skeleton::new(lua, lua.create_table()?)?;

    color::register(&lua, &skeleton)?;
    game_data::register(
        &lua,
        &skeleton,
        conn.clone(),
        thumb_db_path.clone(),
        vt.clone(),
    )?;
    image::register(&lua, conn, thumb_db_path, &skeleton)?;
    cmd::register(&lua, &skeleton, &sink_builder)?;
    misc::register(&lua, &skeleton, vt.clone())?;
    yaml::register(&lua, &skeleton)?;
    path::register(&lua, &skeleton)?;
    dialog::register(&lua, &skeleton, vt.clone())?;

    let Skeleton { module, .. } = &skeleton;

    fs::register(&lua, &module)?;
    print::register(&lua, &module, &sink_builder)?;

    module.set("None", ::mlua::Value::NULL)?;

    module.set("settings", lua.to_value(&vt.settings()?)?)?;

    let module = module.clone();
    let mut available = vt.available_modules();
    let mut loaded = FxHashMap::<String, ::mlua::Value>::default();
    lua.globals().set(
        "require",
        lua.create_function_mut(move |lua, name: String| match name.as_str() {
            "@spel-katalog" | "spel-katalog" => Ok(::mlua::Value::Table(module.clone())),
            other => {
                if let Some(source) = available.remove(other) {
                    let module = lua.load(source).call::<::mlua::Value>(())?;
                    loaded.insert(name, module.clone());
                    Ok(module)
                } else if let Some(module) = loaded.get(other) {
                    Ok(module.clone())
                } else {
                    Err(::mlua::Error::RuntimeError(format!(
                        "could not find module {other:?}"
                    )))
                }
            }
        })?,
    )?;

    Ok(skeleton)
}
