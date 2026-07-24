//! Gather process info.

use ::core::ops::Mul;
use ::std::{ffi::OsStr, io, os::unix::ffi::OsStrExt, path::PathBuf};

use ::iced_core::Length::{self};
use ::iced_widget::{self as widget, button, text, value};
use ::rustc_hash::FxHashSet;
use ::smol::{fs, stream::StreamExt};
use ::spel_katalog_common::{in_place::PushMaybe as _, styling, w};
use ::spel_katalog_widget::Element;
use ::tap::Pipe;

use crate::Message;

/// Level and pid of a process to gather info for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Process {
    /// Level of process in tree.
    level: usize,
    /// Pid of process.
    pid: i64,
}

/// Info of a process in process tree.
#[derive(Debug, Clone)]
pub struct ProcessInfo {
    /// Level of process.
    level: usize,
    /// Pid of process.
    pid: i64,
    /// Name of process.
    name: Option<String>,
    /// Command line of process.
    cmdline: String,
}

/// Collected process info and a list
/// of pids for which info could not be collected.
#[derive(Debug, Clone)]
pub struct CollectedInfo {
    /// Info of processes.
    pub(crate) info: Vec<ProcessInfo>,
    /// Pids for which collection failed.
    pub(crate) failed: Vec<i64>,
}

impl CollectedInfo {
    /// Collect info.
    ///
    /// # Errors
    /// If children of self cannot be gathered.
    pub async fn new(additional_roots: &FxHashSet<i64>) -> io::Result<CollectedInfo> {
        let mut stack = Vec::<Process>::new();
        fs::read_dir("/proc/self/task/")
            .await?
            .filter_map(|entry| entry.ok())
            .then(|entry| async move {
                let path = entry.path().join("children");
                let task_children = match fs::read_to_string(&path).await {
                    Ok(task_children) => task_children,
                    Err(err) => {
                        ::log::error!("reading path {path:?}\n{err}");
                        return None;
                    }
                };

                Some(task_children)
            })
            .for_each(|task_children| {
                let Some(task_children) = task_children else {
                    return;
                };
                for line in task_children.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    let Ok(pid) = line.parse::<i64>() else {
                        continue;
                    };

                    if !additional_roots.contains(&pid) {
                        stack.push(Process { level: 0, pid });
                    }
                }
            })
            .await;

        for root in additional_roots {
            stack.push(Process {
                level: 0,
                pid: *root,
            });
        }

        let mut info = Vec::<ProcessInfo>::new();
        let mut failed = Vec::<i64>::new();
        while let Some(process) = stack.pop() {
            if let Some(process_info) = ProcessInfo::new(process, &mut stack).await {
                info.push(process_info);
            } else {
                failed.push(process.pid);
            }
        }

        Ok(CollectedInfo { info, failed })
    }
}

impl ProcessInfo {
    /// Construct a process info from a pid and level.
    async fn new(process: Process, stack: &mut Vec<Process>) -> Option<Self> {
        let Process { level, pid } = process;
        let proc = PathBuf::from(format!("/proc/{pid}"));

        let status = proc.join("status");
        let name = match fs::read(&status).await {
            Ok(bytes) => {
                let mut name = None;
                for line in bytes.split(|c| *c == b'\n').map(|line| line.trim_ascii()) {
                    if let Some(line) = line.strip_prefix(b"Name:") {
                        name = line
                            .trim_ascii()
                            .pipe(OsStr::from_bytes)
                            .display()
                            .to_string()
                            .pipe(Some);
                        break;
                    }
                }
                name
            }
            Err(err) => {
                ::log::error!("while reading {status:?}\n{err}");
                None
            }
        };

        let cmdline = proc.join("cmdline");

        let mut cmdline = fs::read(&cmdline)
            .await
            .map_err(|err| ::log::error!("while reading {cmdline:?}\n{err}"))
            .ok()?;

        let next_level = level.saturating_add(1);

        while cmdline.last() == Some(&b'\0') {
            cmdline.pop();
        }

        let cmdline = cmdline
            .split(|c| *c == b'\0')
            .map(|bytes| OsStr::from_bytes(bytes).display().to_string())
            .pipe(::shell_words::join);

        let tasks = proc.join("task");
        let tasks = fs::read_dir(&tasks)
            .await
            .map_err(|err| ::log::error!("reading directory {tasks:?}\n{err}"))
            .ok()?;

        tasks
            .filter_map(|entry| entry.ok())
            .then(|entry| async move {
                let path = entry.path().join("children");
                let task_children = match fs::read_to_string(&path).await {
                    Ok(task_children) => task_children,
                    Err(err) => {
                        ::log::error!("reading path {path:?}\n{err}");
                        return None;
                    }
                };
                Some(task_children)
            })
            .for_each(|task_children| {
                if let Some(task_children) = task_children {
                    stack.extend(task_children.lines().flat_map(|line| {
                        let line = line.trim();
                        if line.is_empty() {
                            None
                        } else {
                            line.parse::<i64>().ok().map(|pid| Process {
                                level: next_level,
                                pid,
                            })
                        }
                    }));
                }
            })
            .await;

        Some(ProcessInfo {
            level,
            pid,
            name,
            cmdline,
        })
    }

    /// View a single process info item
    pub fn view(&self) -> Element<'_, Message> {
        let Self {
            level,
            pid,
            name,
            cmdline,
        } = self;
        let pid = *pid;
        let level = *level;

        w::row()
            .spacing(6)
            .push(widget::space::horizontal().width(Length::Fixed(level.min(24).mul(12) as f32)))
            .push(
                button("X")
                    .padding(3)
                    .style(button::danger)
                    .on_press(Message::Terminate { pid }),
            )
            .push(
                button("K")
                    .padding(3)
                    .style(button::secondary)
                    .on_press(Message::Kill { pid }),
            )
            .push(value(pid))
            .push_maybe(name.as_ref().map(text))
            .push(
                text(cmdline)
                    .pipe(widget::container)
                    .padding(3)
                    .style(|t| styling::box_border(t).background(t.palette().background)),
            )
            .pipe(Element::from)
            .map(Into::into)
    }
}
