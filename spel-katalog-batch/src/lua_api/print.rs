//! debug and printing utilities.

use ::std::io::{PipeWriter, Write};

use ::mlua::{Lua, Table};
use ::spel_katalog_terminal::{SinkBuilder, SinkIdentity};

fn lua_print(_: &Lua, value: String, w: Option<&mut PipeWriter>) -> ::mlua::Result<()> {
    if let Some(w) = w {
        writeln!(w, "{value}")?;
    } else {
        println!("{value}");
    }
    Ok(())
}

fn lua_dbg(
    _: &Lua,
    mv: ::mlua::MultiValue,
    w: Option<&mut PipeWriter>,
) -> ::mlua::Result<::mlua::MultiValue> {
    if let Some(w) = w {
        for value in &mv {
            writeln!(w, "{value:#?}")?;
        }
    } else {
        for value in &mv {
            println!("{value:#?}");
        }
    }
    Ok(mv)
}

pub fn register_print(lua: &Lua, module: &Table, sink_builder: &SinkBuilder) -> ::mlua::Result<()> {
    let [mut stdout, mut stderr] = if let Some([stdout, stderr]) =
        sink_builder.get_pipe_writer_double(|| SinkIdentity::StaticName("Lua Batch"))?
    {
        [Some(stdout), Some(stderr)]
    } else {
        [None, None]
    };

    let dbg = lua.create_function_mut(move |lua, value| lua_dbg(lua, value, stderr.as_mut()))?;
    let prnt = lua.create_function_mut(move |lua, value| lua_print(lua, value, stdout.as_mut()))?;

    module.set("dbg", dbg)?;
    module.set("print", prnt)?;
    Ok(())
}
