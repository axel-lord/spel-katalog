use ::std::{path::Path, rc::Rc};

use ::mlua::{Lua, Table};
use ::once_cell::unsync::OnceCell;
use ::rusqlite::Connection;

use crate::{Skeleton, image::Image, make_class, yaml::load_yaml};

pub fn register(
    lua: &Lua,
    skeleton: &Skeleton,
    conn: Rc<OnceCell<Connection>>,
    db_path: Rc<Path>,
) -> ::mlua::Result<()> {
    let game_data = &skeleton.game_data;
    make_class(lua, game_data)?;

    game_data.set(
        "loadConfig",
        lua.create_function(|lua, this: Table| load_yaml(lua, this.get("config")?))?,
    )?;

    let dbp = db_path.clone();
    let cn = conn.clone();
    game_data.set(
        "loadCover",
        lua.create_function(move |_lua, this: Table| {
            Image::load_cover(this.get("slug")?, &dbp, &cn)
        })?,
    )?;

    let dbp = db_path.clone();
    let cn = conn.clone();
    game_data.set(
        "saveCover",
        lua.create_function(move |_lua, (this, image): (Table, Image)| {
            image.save_cover(this.get("slug")?, &dbp, &cn)
        })?,
    )?;

    skeleton.module.set("GameData", game_data)?;

    Ok(())
}
