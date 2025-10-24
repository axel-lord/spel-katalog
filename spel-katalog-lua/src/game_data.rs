use ::mlua::Lua;

use crate::{make_class, Skeleton};

pub fn register(lua: &Lua, skeleton: &Skeleton) -> ::mlua::Result<()> {
    let game_data = &skeleton.game_data;
    make_class(lua, game_data)?;
    Ok(())
}

