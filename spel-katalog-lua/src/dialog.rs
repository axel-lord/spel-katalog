//! Dialog api.

use ::std::sync::Arc;

use ::mlua::{Lua, Table};

use crate::{Skeleton, Virtual, init_table, make_class};

/// Register dialog functions with table.
pub fn register(lua: &Lua, skeleton: &Skeleton, vt: Arc<dyn Virtual>) -> ::mlua::Result<()> {
    let dialog = lua.create_table()?;
    make_class(lua, &dialog)?;

    init_table! {
        dialog:
            text = "",
            buttons = vec!["Ok", "Cancel"],
    }?;

    dialog.set(
        "open",
        lua.create_function(move |_lua, table: Table| {
            let mut result = vt.open_dialog(table.get("text")?, table.get("buttons")?)?;

            if let Some(r) = &result {
                let ignored = table.get::<Vec<String>>("ignore").ok();
                if let Some(ignored) = ignored
                    && ignored.contains(r)
                {
                    result = None;
                }
            }

            Ok(result)
        })?,
    )?;

    skeleton.module.set("Dialog", dialog)?;

    Ok(())
}
