use ::core::time::Duration;
use ::std::{ffi::OsStr, io, ops::Mul, os::unix::ffi::OsStrExt, path::PathBuf, sync::Arc};

use ::iced::{Subscription, Task};
use ::iced_core::{
    Color,
    Length::{self, Fill},
    alignment::Horizontal::Left,
};
use ::iced_futures::backend::default::time::every;
use ::iced_widget::{self as widget, button, container, opaque, text, value};
use ::rustc_hash::FxHashSet;
use ::rustix::process::{Pid, RawPid, Signal, kill_process};
use ::smol::{fs, lock::Semaphore, stream::StreamExt, unblock};
use ::spel_katalog_common::in_place::PushMaybe as _;
use ::spel_katalog_common::{styling, w};
use ::tap::Pipe;

use crate::Element;

/// Send given signal to process.
async fn signal_process(pid: i64, signal: Signal) {
    let pid = match RawPid::try_from(pid) {
        Ok(pid) => pid,
        Err(err) => {
            ::log::error!("could not convert {pid} to a process id\n{err}");
            return;
        }
    };

    let Some(pid) = Pid::from_raw(pid) else {
        ::log::error!("could not convert raw process id {pid} to a non-raw process id");
        return;
    };

    match unblock(move || kill_process(pid, signal)).await {
        Ok(_) => {
            ::log::info!("sent signal {signal:?} to process {pid}")
        }
        Err(err) => {
            ::log::error!("could not send signal {signal:?} to process {pid}\n{err}")
        }
    }
}

/// Display a process tree.
#[derive(Debug)]
pub struct ProcessView {
    /// Displayed processes.
    list: Vec<ProcessInfo>,
    /// Processes other than self to find children for.
    additional: Arc<FxHashSet<i64>>,
    /// Semaophore used to ensure only one
    /// refresh is happening at a time.
    refresh_semaphore: Arc<Semaphore>,
}

impl ProcessView {
    /// Get current additional roots.
    pub fn additional(&self) -> &FxHashSet<i64> {
        &self.additional
    }

    /// Add an additional pid to track.
    pub fn add_pid(&mut self, pid: i64) {
        Arc::make_mut(&mut self.additional).insert(pid);
    }

    /// Refresh view content.
    pub fn refresh(&mut self) -> Task<Message> {
        if let Some(guard) = self.refresh_semaphore.try_acquire_arc() {
            let additional = Arc::clone(&self.additional);
            Task::future(async move {
                let result = CollectedInfo::new(&additional).await;
                drop(guard);
                result
                    .map_err(|err| {
                        ::log::error!("error occured trying to collect process info\n{err}")
                    })
                    .ok()
                    .map(Message::Collected)
            })
            .and_then(Task::done)
        } else {
            Task::none()
        }
    }

    /// Subscribe to refresh messages.
    pub fn subscription(&self) -> Subscription<Message> {
        every(Duration::from_millis(500)).map(|_| Message::Refresh)
    }

    /// Update state using message
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Collected(collected_info) => {
                let CollectedInfo { info, failed } = collected_info;
                self.list = info;
                let additional = Arc::make_mut(&mut self.additional);
                for failed in failed {
                    additional.remove(&failed);
                }
                Task::none()
            }
            Message::Refresh => self.refresh(),
            Message::Kill { pid } => Task::future(signal_process(pid, Signal::KILL)).discard(),
            Message::Terminate { pid } => Task::future(signal_process(pid, Signal::TERM)).discard(),
        }
    }

    /// View process info.
    pub fn view(&self) -> Element<'_, Message> {
        container(
            w::col()
                .push("Process Tree")
                .extend(self.list.iter().map(|info| info.view()))
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
}

impl Default for ProcessView {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            additional: Arc::new(FxHashSet::default()),
            refresh_semaphore: Arc::new(Semaphore::new(1)),
        }
    }
}

/// Message used by process view.
#[derive(Debug, Clone)]
pub enum Message {
    /// Info was collected.
    Collected(CollectedInfo),
    /// Content is to be refreshed.
    Refresh,
    /// Kill a process.
    Kill {
        /// Process to kill.
        pid: i64,
    },
    /// Terminate a process.
    Terminate {
        /// Process to terminate.
        pid: i64,
    },
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    level: usize,
    pid: i64,
    name: Option<String>,
    cmdline: String,
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

impl CollectedInfo {
    /// Collect info.
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
}

impl ProcessInfo {
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
