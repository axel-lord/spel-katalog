use ::mlua::{Lua, Table, Variadic};

pub fn get_class(module: &Table) -> ::mlua::Result<Table> {
    module.get("Color")
}

pub fn new_color(class: &Table, initial: Table) -> ::mlua::Result<Table> {
    initial.set_metatable(Some(class.clone()))?;
    Ok(initial)
}

pub fn register_color(lua: &Lua, module: &Table) -> ::mlua::Result<()> {
    let color = lua.create_table()?;
    color.set("r", 0)?;
    color.set("g", 0)?;
    color.set("b", 0)?;
    color.set("a", 1.0)?;
    color.set("__index", &color)?;
    module.set("Color", &color)?;

    color.set(
        "new",
        lua.create_function(
            move |lua,
                  (class, tables): (Table, Variadic<Table>)|
                  -> ::mlua::Result<Variadic<Table>> {
                if tables.is_empty() {
                    lua.create_table()
                        .and_then(|initial| new_color(&class, initial))
                        .map(|color| Variadic::from_iter([color]))
                } else {
                    tables
                        .into_iter()
                        .map(|initial| new_color(&class, initial))
                        .collect::<Result<Variadic<_>, _>>()
                }
            },
        )?,
    )?;

    Ok(())
}
