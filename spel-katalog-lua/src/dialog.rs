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
            let mut result = dialog_opener(table.get("text")?, table.get("buttons")?)?;

            if let Some(r) = &result {
                let ignored = table.get::<Vec<String>>("ignore").ok();
                if let Some(ignored) = ignored {
                    if ignored.contains(r) {
                        result = None;
                    }
                }
            }

            Ok(result)
        })?,
    )?;

    skeleton.module.set("Dialog", dialog)?;

    Ok(())
}
