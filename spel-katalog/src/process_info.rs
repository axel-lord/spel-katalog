use ::std::{ffi::OsStr, io, ops::Mul, os::unix::ffi::OsStrExt, path::PathBuf};

use ::iced::{
    Color, Element,
    Length::{self, Fill},
    alignment::Horizontal::Left,
    widget::{self, button, container, horizontal_space, opaque, text, value},
};
use ::spel_katalog_common::{styling, w};
use ::tap::Pipe;

use crate::Message;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub(crate) level: usize,
    pub(crate) pid: i64,
    pub(crate) name: Option<String>,
    pub(crate) cmdline: String,
}

impl ProcessInfo {
    pub fn view_list<'e>(list: &'e [ProcessInfo]) -> Element<'e, Message> {
        container(
            w::col()
                .push("Process Tree")
                .extend(list.iter().map(|info| info.view()))
                .align_x(Left)
                .pipe(w::scroll)
                .pipe(container)
                .style(container::bordered_box)
                .padding(3),
        )
        .center(Fill)
        .style(|_theme| container::background(Color::from_rgba8(0, 0, 0, 0.7)))
        .pipe(opaque)
        .into()
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
        w::row()
            .spacing(6)
            .push(horizontal_space().width(Length::Fixed(level.min(24).mul(12) as f32)))
            .push(
                button("X")
                    .padding(3)
                    .style(button::danger)
                    .on_press_with(move || Message::Kill {
                        pid,
                        terminate: true,
                    }),
            )
            .push(
                button("K")
                    .padding(3)
                    .style(button::secondary)
                    .on_press_with(move || Message::Kill {
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
            .into()
    }

    pub async fn open() -> Result<Vec<ProcessInfo>, io::Error> {
        let mut task_dir = ::tokio::fs::read_dir("/proc/self/task/").await?;
        let mut children = Vec::new();
        while let Some(entry) = task_dir.next_entry().await? {
            let path = entry.path().join("children");
            let task_children = match ::tokio::fs::read_to_string(&path).await {
                Ok(task_children) => task_children,
                Err(err) => {
                    ::log::error!("reading path {path:?}\n{err}");
                    continue;
                }
            };

            children.extend(task_children.lines().flat_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    None
                } else {
                    line.parse::<i64>().ok().map(|i| (0usize, i))
                }
            }));
        }

        let mut summary = Vec::<ProcessInfo>::new();
        while let Some((level, child)) = children.pop() {
            let proc = PathBuf::from(format!("/proc/{child}"));

            let status = proc.join("status");
            let name = match ::tokio::fs::read(&status).await {
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

            let mut cmdline = match ::tokio::fs::read(&cmdline).await {
                Ok(cmdline) => cmdline,
                Err(err) => {
                    ::log::error!("while reading {cmdline:?}\n{err}");
                    continue;
                }
            };

            let next_level = level.saturating_add(1);

            while cmdline.last() == Some(&b'\0') {
                cmdline.pop();
            }

            let cmdline = cmdline
                .split(|c| *c == b'\0')
                .map(|bytes| OsStr::from_bytes(bytes).display().to_string())
                .pipe(::shell_words::join);

            let tasks = proc.join("task");
            let mut tasks = match ::tokio::fs::read_dir(&tasks).await {
                Ok(tasks) => tasks,
                Err(err) => {
                    ::log::error!("reading directory {tasks:?}\n{err}");
                    continue;
                }
            };

            while let Some(entry) = tasks.next_entry().await? {
                let path = entry.path().join("children");
                let task_children = match ::tokio::fs::read_to_string(&path).await {
                    Ok(task_children) => task_children,
                    Err(err) => {
                        ::log::error!("reading path {path:?}\n{err}");
                        continue;
                    }
                };

                children.extend(task_children.lines().flat_map(|line| {
                    let line = line.trim();
                    if line.is_empty() {
                        None
                    } else {
                        line.parse::<i64>().ok().map(|i| (next_level, i))
                    }
                }));
            }

            summary.push(ProcessInfo {
                level,
                pid: child,
                name,
                cmdline,
            });
        }

        Ok::<_, ::tokio::io::Error>(summary)
    }
}
