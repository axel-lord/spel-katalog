//! debug and printing utilities.

use ::std::io::{self, PipeWriter, Write};

use ::either::{Either, Left, Right};
use ::mlua::{Lua, Table};
use ::spel_katalog_sink::{SinkBuilder, SinkIdentity};

fn lua_print(
    _: &Lua,
    mv: ::mlua::MultiValue,
    to_string: &::mlua::Function,
    mut w: Either<&mut io::Stdout, &mut PipeWriter>,
) -> ::mlua::Result<()> {
    for value in mv {
        let content = to_string.call::<::mlua::String>(value)?;
        w.write_all(&content.as_bytes())?;
    }
    w.write_all(b"\n")?;
    Ok(())
}

fn lua_dbg(
    _: &Lua,
    mv: ::mlua::MultiValue,
    mut w: Either<&mut io::Stderr, &mut PipeWriter>,
) -> ::mlua::Result<::mlua::MultiValue> {
    for value in &mv {
        writeln!(w, "{value:#?}")?;
    }
    Ok(mv)
}

pub fn register(lua: &Lua, module: &Table, sink_builder: &SinkBuilder) -> ::mlua::Result<()> {
    let to_string = lua.globals().get("tostring")?;

    let (mut stdout, mut stderr) = sink_builder
        .get_pipe_writer_double(|| SinkIdentity::StaticName("Lua Print"))?
        .map_or_else(
            || (Left(io::stdout()), Left(io::stderr())),
            |[stdout, stderr]| (Right(stdout), Right(stderr)),
        );

    let dbg = lua.create_function_mut(move |lua, value| lua_dbg(lua, value, stderr.as_mut()))?;
    let prnt = lua.create_function_mut(move |lua, value| {
        lua_print(lua, value, &to_string, stdout.as_mut())
    })?;

    module.set("dbg", dbg)?;
    module.set("print", &prnt)?;
    lua.globals().set("print", prnt)?;

    Ok(())
}
