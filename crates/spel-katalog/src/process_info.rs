use ::std::{ffi::OsStr, io, ops::Mul, os::unix::ffi::OsStrExt, path::PathBuf};

use ::iced_core::{
    Color,
    Length::{self, Fill},
    alignment::Horizontal::Left,
};
use ::iced_widget::{self as widget, button, container, opaque, text, value};
use ::rustc_hash::FxHashSet;
use ::smol::stream::StreamExt;
use ::spel_katalog_common::in_place::PushMaybe as _;
use ::spel_katalog_common::{styling, w};
use ::tap::Pipe;

use crate::{Element, Message};

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub(crate) level: usize,
    pub(crate) pid: i64,
    pub(crate) name: Option<String>,
    pub(crate) cmdline: String,
}

/// Collected process info and a list
/// of pids for which info could not be collected.
#[derive(Debug, Clone)]
pub struct CollectedInfo {
    /// Info of processes.
    pub info: Vec<ProcessInfo>,
    /// Pids for which collection failed.
    pub failed: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Process {
    level: usize,
    pid: i64,
}

impl ProcessInfo {
    /// Construct a process info from a pid and level.
    async fn new(process: Process, stack: &mut Vec<Process>) -> Option<Self> {
        let Process { level, pid } = process;
        let proc = PathBuf::from(format!("/proc/{pid}"));

        let status = proc.join("status");
        let name = match ::smol::fs::read(&status).await {
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

        let mut cmdline = ::smol::fs::read(&cmdline)
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
        let tasks = ::smol::fs::read_dir(&tasks)
            .await
            .map_err(|err| ::log::error!("reading directory {tasks:?}\n{err}"))
            .ok()?;

        tasks
            .filter_map(|entry| entry.ok())
            .then(|entry| async move {
                let path = entry.path().join("children");
                let task_children = match ::smol::fs::read_to_string(&path).await {
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
}

impl ProcessInfo {
    pub fn view_list<'e>(list: &'e [ProcessInfo]) -> Element<'e, Message> {
        container(
            w::col()
                .push("Process Tree")
                .extend(list.iter().map(|info| info.view()))
                .align_x(Left)
                .pipe(spel_katalog_widget::scrollable)
                .pipe(container)
                .style(container::bordered_box)
                .padding(3),
        )
        .center(Fill)
        .style(|_theme| container::background(Color::from_rgba8(0, 0, 0, 0.7)))
        .pipe(opaque)
    }

    pub fn view<'e>(&'e self) -> Element<'e, Message> {
        let Self {
            level,
            pid,
            name,
            cmdline,
        } = self;
        let pid = *pid;
        let level = *level;

        #[derive(Debug, Clone, Copy)]
        struct Kill {
            pid: i64,
            terminate: bool,
        }

        impl From<Kill> for Message {
            fn from(value: Kill) -> Self {
                let Kill { pid, terminate } = value;
                Message::Kill { pid, terminate }
            }
        }

        w::row()
            .spacing(6)
            .push(widget::space::horizontal().width(Length::Fixed(level.min(24).mul(12) as f32)))
            .push(button("X").padding(3).style(button::danger).on_press(Kill {
                pid,
                terminate: true,
            }))
            .push(
                button("K")
                    .padding(3)
                    .style(button::secondary)
                    .on_press(Kill {
                        pid,
                        terminate: false,
                    }),
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

    pub async fn open(additional_roots: &FxHashSet<i64>) -> io::Result<CollectedInfo> {
        let mut stack = Vec::<Process>::new();
        ::smol::fs::read_dir("/proc/self/task/")
            .await?
            .filter_map(|entry| entry.ok())
            .then(|entry| async move {
                let path = entry.path().join("children");
                let task_children = match ::smol::fs::read_to_string(&path).await {
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
