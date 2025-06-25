//! Executables to run.

use ::std::{
    fmt::Display,
    process::{ExitStatus, Stdio},
    str::FromStr,
    time::Duration,
};

use ::bon::Builder;
use ::derive_more::From;
use ::serde::{Deserialize, Serialize};
use ::tokio::{process::Command, time::timeout};

use crate::{builder_push::builder_push, environment::Env};

/// Error that occurs on failure while running a command.
#[derive(Debug, ::thiserror::Error)]
pub enum ExecError {
    /// Error that occcurs when a process cannot be spawned.
    #[error("process could not be spawned, {0}")]
    Spawn(#[source] ::std::io::Error),
    /// Errpr that occurs on timeour.
    #[error("process did not finishe withing {d:.2} seconds, {err}", d = .1.as_secs_f64(), err = .0)]
    Timeout(#[source] ::tokio::time::error::Elapsed, Duration),
    /// Error that occurs on wait failure.
    #[error("process could not be waited, {0}")]
    Wait(#[source] ::std::io::Error),
    /// Error that occues when a process cannot be killed.
    #[error("process could not be killed, {0}")]
    Kill(#[source] ::std::io::Error),
}

/// Some executable.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, From)]
#[serde(untagged)]
pub enum Exec {
    /// Exec is a command, that should be split.
    Cmd(Cmd),
    /// Exec is a program, (exec + args)
    Program(Program),
}

impl Exec {
    /// Visit all parsed string values.
    pub fn visit_strings<E>(
        &mut self,
        f: impl FnMut(&mut String) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Exec::Cmd(Cmd { cmd }) => cmd.iter_mut().try_for_each(f),
            Exec::Program(program) => program.visit_strings(f),
        }
    }

    /// Run executable.
    pub async fn run(&self, env: &Env) -> Result<ExitStatus, ExecError> {
        let exec;
        let mut args;
        match self {
            Exec::Cmd(Cmd { cmd }) => {
                args = cmd.iter();
                exec = args.next().unwrap_or_else(|| unreachable!());
            }
            Exec::Program(Program { exec: e, args: a }) => {
                exec = e;
                args = a.iter();
            }
        }

        let mut command = Command::new(exec);
        command.args(args);

        if env.unset_all {
            command.env_clear();
        } else {
            for key in &env.unset {
                command.env_remove(key);
            }
        }
        command.envs(&env.vars);

        command
            .kill_on_drop(true)
            .stdin(Stdio::inherit())
            .stderr(Stdio::inherit())
            .stdout(Stdio::inherit());

        let mut child = command.spawn().map_err(ExecError::Spawn)?;

        let duration = Duration::from_secs(30);
        let status = match timeout(duration, child.wait()).await {
            Ok(result) => match result {
                Ok(status) => status,
                Err(err) => return Err(ExecError::Wait(err)),
            },
            Err(err) => {
                if let Err(err) = child.kill().await {
                    return Err(ExecError::Kill(err));
                }
                return Err(ExecError::Timeout(err, duration));
            }
        };

        Ok(status)
    }
}

/// A single executable commadn.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, From)]
#[serde(try_from = "Cmdline", into = "Cmdline")]
pub struct Cmd {
    /// Binary to execute.
    pub cmd: Vec<String>,
}

/// A single executable item.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Builder)]
pub struct Program {
    /// Binary to execute.
    #[builder(start_fn, into)]
    pub exec: String,

    /// Arguments to give executable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(field)]
    pub args: Vec<String>,
}

impl Program {
    /// Visit all parsed string values.
    pub fn visit_strings<E>(
        &mut self,
        mut f: impl FnMut(&mut String) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self { exec, args } = self;
        f(exec)?;
        args.iter_mut().try_for_each(f)
    }
}

builder_push!(ProgramBuilder { args, arg: impl Into<String> => arg.into()});

/// Implementation detail for cmd.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct Cmdline {
    cmd: String,
}

/// Error returned when failing to parse a command.
#[derive(Debug, Clone, PartialEq, Eq, ::thiserror::Error)]
pub enum CmdParseError {
    /// A forwarded split error.
    #[error(transparent)]
    Split(#[from] ::shell_words::ParseError),
    /// Parsed line was empty.
    #[error("commands should contain at least 1 compnent")]
    Empty,
}

impl TryFrom<Cmdline> for Cmd {
    type Error = CmdParseError;

    fn try_from(value: Cmdline) -> Result<Self, Self::Error> {
        value.cmd.parse()
    }
}

impl From<Cmd> for Cmdline {
    fn from(value: Cmd) -> Self {
        Self {
            cmd: value.to_string(),
        }
    }
}

impl FromStr for Cmd {
    type Err = CmdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cmd = ::shell_words::split(s)?;
        if cmd.is_empty() {
            return Err(CmdParseError::Empty);
        }
        Ok(Self { cmd })
    }
}

impl Display for Cmd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&String::from(self))
    }
}

impl From<Cmd> for String {
    fn from(value: Cmd) -> Self {
        String::from(&value)
    }
}

impl From<&Cmd> for String {
    fn from(value: &Cmd) -> Self {
        ::shell_words::join(&value.cmd)
    }
}
