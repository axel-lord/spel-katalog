use ::std::{path::Path, rc::Rc, sync::Arc};

use ::mlua::{Lua, Table};
use ::once_cell::unsync::OnceCell;
use ::rusqlite::Connection;

use crate::{
    Skeleton, Virtual,
    image::Image,
    make_class,
    misc::{set_attr, set_attrs},
    yaml::load_yaml,
};

pub fn register(
    lua: &Lua,
    skeleton: &Skeleton,
    conn: Rc<OnceCell<Connection>>,
    db_path: Rc<Path>,
    vt: Arc<dyn Virtual>,
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

    let vt1 = vt.clone();
    game_data.set(
        "setAttr",
        lua.create_function(move |lua, (this, attr, value): (Table, _, _)| {
            let game_id = this.get("id")?;
            let new_attrs = set_attr(lua, game_id, attr, value, vt1.as_ref())?;
            this.set("attrs", new_attrs)?;
            Ok(())
        })?,
    )?;

    game_data.set(
        "setAttrs",
        lua.create_function(move |lua, (this, attrs): (Table, _)| {
            let game_id = this.get("id")?;
            let new_attrs = set_attrs(lua, game_id, attrs, vt.as_ref())?;
            this.set("attrs", new_attrs)?;
            Ok(())
        })?,
    )?;

    skeleton.module.set("GameData", game_data)?;

    Ok(())
}
