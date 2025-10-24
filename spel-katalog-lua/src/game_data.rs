use ::mlua::Lua;

use crate::{Skeleton, make_class};

pub fn register(lua: &Lua, skeleton: &Skeleton) -> ::mlua::Result<()> {
    let game_data = &skeleton.game_data;
    make_class(lua, game_data)?;
    Ok(())
}
