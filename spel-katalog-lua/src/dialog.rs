use ::mlua::{Lua, Table};

use crate::{DialogOpener, Skeleton, create_class, init_table};

pub fn register(
    lua: &Lua,
    skeleton: &Skeleton,
    dialog_opener: Box<DialogOpener>,
) -> ::mlua::Result<()> {
    let dialog = create_class(lua)?;

    init_table! {
        dialog:
            text = "",
            buttons = vec!["Ok", "Cancel"],
    }?;

    dialog.set(
        "open",
        lua.create_function(move |_lua, table: Table| {
            Ok(dialog_opener(table.get("text")?, table.get("buttons")?))
        })?,
    )?;

    skeleton.module.set("Dialog", dialog)?;

    Ok(())
}
