//! Gather process info.

use ::core::{cell::OnceCell, ops::Mul};
use ::std::{
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

use ::iced_core::{Alignment::Center, Color, Length};
use ::iced_widget::{self as widget, text, value};
use ::rustc_hash::FxHashSet;
use ::smol::{fs, stream::StreamExt};
use ::spel_katalog_assets as assets;
use ::spel_katalog_common::in_place::PushMaybe as _;
use ::spel_katalog_widget::{Element, WidgetExt, icon};
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
    /// Split command line.
    split: OnceCell<Option<Vec<String>>>,
}

impl PartialEq for ProcessInfo {
    fn eq(&self, other: &Self) -> bool {
        self.level == other.level
            && self.pid == other.pid
            && self.name == other.name
            && self.cmdline == other.cmdline
    }
}

impl Eq for ProcessInfo {}

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
                if !matches!(err.kind(), ::std::io::ErrorKind::NotFound) {
                    ::log::error!("while reading {status:?}\n{err}");
                }
                None
            }
        };

        let cmdline = proc.join("cmdline");

        let mut cmdline = fs::read(&cmdline)
            .await
            .map_err(|err| {
                if !matches!(err.kind(), ::std::io::ErrorKind::NotFound) {
                    ::log::error!("while reading {cmdline:?}\n{err}");
                }
            })
            .ok()
            .unwrap_or_default();

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
            .map_err(|err| {
                if !matches!(err.kind(), ::std::io::ErrorKind::NotFound) {
                    ::log::error!("reading directory {tasks:?}\n{err}");
                }
            })
            .ok()?;

        tasks
            .filter_map(|entry| entry.ok())
            .then(|entry| async move {
                let path = entry.path().join("children");
                let task_children = match fs::read_to_string(&path).await {
                    Ok(task_children) => task_children,
                    Err(err) => {
                        if !matches!(err.kind(), ::std::io::ErrorKind::NotFound) {
                            ::log::error!("reading path {path:?}\n{err}");
                        }
                        return None;
                    }
                };
                Some(task_children)
            })
            .for_each(|task_children| {
                if let Some(task_children) = task_children {
                    stack.extend(task_children.split_ascii_whitespace().flat_map(|line| {
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
            split: OnceCell::new(),
        })
    }

    /// Split command line.
    fn split_cmdline(&self) -> Option<&[String]> {
        self.split
            .get_or_init(|| {
                ::shell_words::split(&self.cmdline)
                    .map(|args| {
                        args.into_iter()
                            .map(|arg| ::shell_words::quote(&arg).into_owned())
                            .collect()
                    })
                    .ok()
            })
            .as_deref()
    }

    /// Add command line items to row.
    fn add_cmdline<'a>(&'a self, row: widget::Row<'a, Message>) -> widget::Row<'a, Message> {
        let Self { cmdline, .. } = self;

        row.push(
            icon::Icon::new(assets::copy())
                .size(14)
                .into_outline_button(icon::outline::success)
                .on_press_with(|| Message::Copy(cmdline.clone()))
                .with_text_tooltip("Copy Command Line"),
        )
        .push(
            widget::text(cmdline)
                .size(14)
                .wrapping(text::Wrapping::None)
                .pipe(widget::container)
                .style(widget::container::rounded_box)
                .padding(3)
                .clip(true)
                .with_tooltip(
                    if let Some(cmd) = self.split_cmdline() {
                        Element::from(cmd.iter().fold(
                            ::iced_aw::Wrap::new().line_spacing(0).spacing(6),
                            |col, arg| col.push(widget::text(arg)),
                        ))
                    } else {
                        Element::from(widget::text(cmdline))
                    },
                    600,
                ),
        )
    }

    /// View a single process info item
    pub fn view(&self) -> Element<'_, Message> {
        let Self {
            level,
            pid,
            name,
            cmdline: _,
            split: _,
        } = self;
        let pid = *pid;
        let level = *level;

        widget::Row::new()
            .spacing(6)
            .align_y(Center)
            .push(widget::space::horizontal().width(Length::Fixed(level.min(24).mul(12) as f32)))
            .push(
                icon::Icon::new(assets::minus())
                    .size(14)
                    .into_outline_button(icon::outline::danger)
                    .on_press(Message::Terminate { pid })
                    .with_text_tooltip("Request Termination of Process"),
            )
            .push(
                icon::Icon::new(assets::cross())
                    .size(14)
                    .into_outline_button(|_, _| Color::BLACK)
                    .on_press(Message::Kill { pid })
                    .with_text_tooltip("Force Kill Process"),
            )
            .push(
                icon::Icon::new(assets::copy())
                    .size(14)
                    .into_outline_button(icon::outline::success)
                    .on_press_with(move || Message::Copy(pid.to_string()))
                    .with_text_tooltip("Copy Process Id"),
            )
            .push(value(pid).size(14))
            .push_maybe(name.as_ref().map(text).map(|t| t.size(14)))
            .pipe(|row| self.add_cmdline(row))
            .pipe(Element::from)
            .map(Into::into)
    }
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
    pub async fn new(additional_roots: &FxHashSet<i64>) -> CollectedInfo {
        let mut stack = Vec::<Process>::new();
        let task_dir = Path::new("/proc/self/task/");
        match fs::read_dir(task_dir).await {
            Ok(read_self) => {
                read_self
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
                        for line in task_children.split_ascii_whitespace() {
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
            }
            Err(err) => {
                ::log::error!("could not read directory {task_dir:?}\n{err}");
            }
        }

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

        CollectedInfo { info, failed }
    }
}
