//! View processes.

use ::core::time::Duration;
use ::std::sync::Arc;

use ::iced_core::{Color, Length::Fill, alignment::Horizontal::Left};
use ::iced_futures::{Subscription, backend::default::time::every};
use ::iced_runtime::Task;
use ::iced_widget::{container, opaque};
use ::rustc_hash::FxHashSet;
use ::rustix::process::{Pid, RawPid, Signal, kill_process};
use ::smol::{lock::Semaphore, unblock};
use ::spel_katalog_common::w;
use ::spel_katalog_widget::Element;
use ::tap::Pipe;

use crate::info::{CollectedInfo, ProcessInfo};

mod info;

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
    /// Copy process command line.
    Copy(String),
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

impl Default for ProcessView {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            additional: Arc::new(FxHashSet::default()),
            refresh_semaphore: Arc::new(Semaphore::new(1)),
        }
    }
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
            Message::Copy(contents) => ::iced_runtime::clipboard::write(contents),
        }
    }

    /// View process info.
    pub fn view(&self) -> Element<'_, Message> {
        container(
            w::col()
                .extend(self.list.iter().map(|info| info.view()))
                .align_x(Left)
                .padding(3)
                .pipe(spel_katalog_widget::xy_scrollable)
                .width(Fill)
                .height(Fill),
        )
        .style(|_theme| container::background(Color::from_rgba8(0, 0, 0, 0.7)))
        .pipe(opaque)
    }
}
