use ::std::{
    ffi::{OsStr, OsString},
    io::{Read, Write, pipe},
    os::unix::ffi::OsStrExt,
    process, thread,
};

use ::mlua::{IntoLua, Lua, Table, UserDataMethods, Variadic};
use ::spel_katalog_terminal::{SinkBuilder, SinkIdentity};
use ::tap::Pipe;

#[derive(Debug, Clone)]
struct Command {
    exec: OsString,
    args: Vec<OsString>,
}

impl IntoLua for Command {
    fn into_lua(self, lua: &Lua) -> mlua::Result<mlua::Value> {
        lua.create_any_userdata(self).map(::mlua::Value::UserData)
    }
}

impl Command {
    fn new(exec: ::mlua::String, args: Variadic<::mlua::String>) -> ::mlua::Result<Command> {
        let cmd = Command {
            exec: OsString::from(OsStr::from_bytes(&exec.as_bytes())),
            args: args
                .iter()
                .map(|arg| OsString::from(OsStr::from_bytes(&arg.as_bytes())))
                .collect(),
        };

        Ok(cmd)
    }

    fn split_exec(&self) -> ::mlua::Result<Command> {
        let mut initial = self
            .exec
            .as_bytes()
            .pipe(str::from_utf8)
            .map_err(::mlua::Error::external)?
            .pipe(::shell_words::split)
            .map_err(::mlua::Error::external)?
            .into_iter();

        let exec = initial
            .next()
            .ok_or_else(|| ::mlua::Error::runtime("could not split exec of command"))?
            .into();

        let args = initial
            .map(OsString::from)
            .chain(self.args.iter().cloned())
            .collect();

        Ok(Self { exec, args })
    }

    fn output(&self, lua: &Lua, input: Variadic<String>) -> ::mlua::Result<::mlua::Table> {
        let mut input = input.into_iter();
        let table = lua.create_table()?;

        let (status, stdout, stderr) = if let Some(first) = input.next() {
            let (stdin, mut w_stdin) = pipe()?;
            let (mut r_stdout, stdout) = pipe()?;
            let (mut r_stderr, stderr) = pipe()?;

            let mut child = process::Command::new(&self.exec)
                .args(&self.args)
                .stdin(stdin)
                .stdout(stdout)
                .stderr(stderr)
                .spawn()?;

            let r = thread::scope(|s| -> ::std::io::Result<_> {
                let stdout = s.spawn(move || -> ::std::io::Result<_> {
                    let mut buf = Vec::new();
                    r_stdout.read_to_end(&mut buf)?;
                    Ok(buf)
                });

                let stderr = s.spawn(move || -> ::std::io::Result<_> {
                    let mut buf = Vec::new();
                    r_stderr.read_to_end(&mut buf)?;
                    Ok(buf)
                });

                w_stdin.write_all(&first.as_bytes())?;

                for s in input {
                    w_stdin.write_all(b"\n")?;
                    w_stdin.write_all(&s.as_bytes())?;
                }

                let status = child.wait()?;

                let stdout = stdout
                    .join()
                    .unwrap_or_else(|p| ::std::panic::resume_unwind(p))?;

                let stderr = stderr
                    .join()
                    .unwrap_or_else(|p| ::std::panic::resume_unwind(p))?;

                Ok((status.code(), stdout, stderr))
            });

            match r {
                Ok(value) => value,
                Err(err) => {
                    child.kill()?;
                    return Err(err.into());
                }
            }
        } else {
            let process::Output {
                status,
                stdout,
                stderr,
            } = process::Command::new(&self.exec)
                .args(&self.args)
                .output()?;

            (status.code(), stdout, stderr)
        };

        table.set("status", status)?;
        table.set("stdout", lua.create_string(stdout)?)?;
        table.set("stderr", lua.create_string(stderr)?)?;

        Ok(table)
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
        lua.create_function(|_lua, (exec, args)| Command::new(exec, args))?,
    )?;

    lua.register_userdata_type::<Command>(move |r| {
        r.add_method("status", move |_lua, this, _: ()| {
            this.status(&sink_builder)
        });
        r.add_method("output", |lua, this, input| this.output(lua, input));
        r.add_method("splitExec", |_lua, this, _: ()| this.split_exec());
    })?;

    Ok(())
}
