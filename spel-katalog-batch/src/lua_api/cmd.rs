use ::std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::OsStrExt,
    process,
};

use ::mlua::{Lua, Table, UserDataMethods, Variadic};
use ::spel_katalog_terminal::{SinkBuilder, SinkIdentity};
use ::tap::Pipe;

#[derive(Debug, Clone)]
struct Command {
    exec: OsString,
    args: Vec<OsString>,
}

impl Command {
    fn new(
        lua: &Lua,
        exec: ::mlua::String,
        args: Variadic<::mlua::String>,
    ) -> ::mlua::Result<::mlua::Value> {
        let cmd = Command {
            exec: OsString::from(OsStr::from_bytes(&exec.as_bytes())),
            args: args
                .iter()
                .map(|arg| OsString::from(OsStr::from_bytes(&arg.as_bytes())))
                .collect(),
        };

        lua.create_any_userdata(cmd)?
            .pipe(::mlua::Value::UserData)
            .pipe(Ok)
    }

    fn status(&self, sink_builder: &SinkBuilder) -> ::mlua::Result<Option<i32>> {
        let [stdout, stderr] =
            sink_builder.build_double(|| SinkIdentity::StaticName("Lua Batch Cmd"))?;
        Ok(process::Command::new(&self.exec)
            .args(&self.args)
            .stdout(stdout)
            .stderr(stderr)
            .status()?
            .code())
    }
}

pub fn register_cmd(lua: &Lua, module: &Table, sink_builder: &SinkBuilder) -> ::mlua::Result<()> {
    let sink_builder = sink_builder.clone();

    module.set(
        "cmd",
        lua.create_function(|lua, (exec, args)| Command::new(lua, exec, args))?,
    )?;

    lua.register_userdata_type::<Command>(move |r| {
        r.add_method("status", move |_lua, this, _: ()| {
            this.status(&sink_builder)
        });
    })?;

    Ok(())
}
