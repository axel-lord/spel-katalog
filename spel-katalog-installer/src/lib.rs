//! Utility to install new native games.

use ::std::{borrow::Cow, os::unix::fs::MetadataExt, path::PathBuf};

use ::derive_more::From;
use ::iced_core::Element;
use ::iced_runtime::Task;
use ::iced_widget::{self as widget};
use ::smol::stream::StreamExt;
use ::spel_katalog_common::OrRequest;
use ::spel_katalog_formats::NativeGame;
use ::spel_katalog_settings::{InstallSource, Settings};
use ::tap::TapOptional;

use crate::prepare::ExeChoice;

mod editor;
mod prepare;

/// Message used by installer.
#[derive(Debug, Clone, From)]
pub enum Message {
    /// Editor message.
    #[from]
    Editor(editor::Message),
    /// Prepare message.
    #[from]
    Prepare(prepare::Message),
    /// Open game selection.
    SelectDir(Option<PathBuf>),
    /// Open game selection.
    SelectFile,
    /// Change state to prepare, and set exe list
    SetPaths {
        /// Game directory, parent of executable, or
        /// where list was found from.
        parent: String,
        /// Value of executable.
        choice: ExeChoice,
    },
    /// Set state to editor for game.
    SetEditor(Box<NativeGame>),
    /// Return to prepare stage.
    UnsetEditor,
}

/// Window request.
#[derive(Debug, Clone)]
pub enum Request {
    /// Request the installer be closed.
    Close,
    /// Request game to be added.
    InstallGame {
        /// Game config.
        config: Box<NativeGame>,
        /// Thumbnail of game.
        thumbnail: Option<::spel_katalog_formats::Image>,
        /// Location to move game to.
        move_dir: Option<(PathBuf, PathBuf)>,
    },
}

/// Application installer.
#[derive(Debug, Clone)]
pub struct Installer {
    /// Preparation stage.
    prepare: prepare::Prepare,
    /// Editor is shown.
    editor: Option<editor::Editor>,
}

impl Installer {
    /// Construct a new installer.
    pub fn new(
        settings: &Settings,
        parent: String,
        choice: ExeChoice,
        hidden: Option<bool>,
        thumbnail: Option<PathBuf>,
        move_game: Option<bool>,
    ) -> (Self, Task<Message>) {
        let (prepare, task) =
            prepare::Prepare::new(settings, parent, choice, hidden, thumbnail, move_game);
        (
            Self {
                prepare,
                editor: None,
            },
            task.map(Message::Prepare),
        )
    }

    /// Show open dialog.
    pub async fn open(initial_dir: PathBuf) -> Option<(String, ExeChoice)> {
        let mut dialog =
            ::rfd::AsyncFileDialog::new().set_title("Select directory of game to install");
        if !initial_dir.as_os_str().is_empty() {
            dialog = dialog.set_directory(&initial_dir);
        }

        let directory = dialog
            .pick_folder()
            .await
            .tap_none(|| ::log::warn!("no game directory selected"))?;

        Self::open_path(directory.path().to_path_buf()).await
    }

    /// Open given directory.
    pub async fn open_path(directory: PathBuf) -> Option<(String, ExeChoice)> {
        let mut stack = vec![Cow::Borrowed(directory.as_path())];
        let mut exe = Vec::<String>::new();
        let mut push_exe = |entry_path: PathBuf| {
            match entry_path.strip_prefix(&directory) {
                Ok(rel_path) => {
                    if let Some(as_str) = rel_path.to_str() {
                        exe.push(as_str.to_owned());
                    } else {
                        ::log::warn!("ignoring non utf-8 path {entry_path:?}");
                    }
                }
                Err(err) => ::log::error!(
                    "could not remove prefix {:?} from {:?}\n{err}",
                    &directory,
                    entry_path
                ),
            };
        };
        while let Some(directory) = stack.pop() {
            let path = &*directory;
            let Ok(mut dir) = ::smol::fs::read_dir(path)
                .await
                .map_err(|err| ::log::error!("could not read directory {path:?}\n{err}"))
            else {
                continue;
            };

            while let Some(entry) = dir.next().await {
                let Ok(entry) = entry.map_err(|err| {
                    ::log::error!("could not get directory entry for child of {path:?}\n{err}")
                }) else {
                    continue;
                };
                let entry_path = entry.path();

                let Ok(t) = entry.file_type().await.map_err(|err| {
                    ::log::error!("could not get file type for {entry_path:?}\n{err}")
                }) else {
                    continue;
                };

                if t.is_symlink() {
                    ::log::info!("skipping symlink {entry_path:?}");
                    continue;
                } else if t.is_dir() {
                    if entry_path
                        .file_name()
                        .and_then(|s| s.as_encoded_bytes().first())
                        .is_some_and(|&f| f == b'.')
                    {
                        ::log::info!("skipping hidden directory {entry_path:?}");
                    } else {
                        stack.push(Cow::Owned(entry_path));
                    }
                    continue;
                } else if !t.is_file() {
                    ::log::info!("found non file/symlink/directory {entry_path:?}");
                    continue;
                }

                let meta = entry
                    .metadata()
                    .await
                    .map_err(|err| {
                        ::log::error!("could not get metadata for {entry_path:?}\n{err}")
                    })
                    .ok();

                if entry_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
                {
                    push_exe(entry_path);
                    continue;
                }

                if meta.is_some_and(|m| m.mode() & 0o111 != 0) {
                    push_exe(entry_path);
                }
            }
        }

        if exe.is_empty() {
            ::log::warn!("no exe candidates found");
            None
        } else {
            ::log::info!("collected exe candidates");
            let parent = directory
                .into_os_string()
                .into_string()
                .map_err(|path| {
                    ::log::error!("directory path {path:?} contains non utf-8 segments")
                })
                .ok()?;

            exe.sort_unstable_by_key(|item| item.len());
            Some((parent, ExeChoice::List(0, exe)))
        }
    }

    /// Update state using message.
    pub fn update(
        &mut self,
        message: Message,
        settings: &Settings,
    ) -> Task<OrRequest<Message, Request>> {
        match message {
            Message::SetEditor(game) => {
                self.editor = editor::Editor::new(&game);
                Task::none()
            }
            Message::UnsetEditor => {
                self.editor = None;
                Task::none()
            }
            Message::Editor(message) => {
                if let Some(editor) = &mut self.editor {
                    editor.update(
                        message,
                        self.prepare.thumbnail(),
                        self.prepare.move_game(),
                        &|| self.prepare.parent().to_path_buf(),
                        &|| self.prepare.game_dir(settings),
                    )
                } else {
                    ::log::warn!("received editor message whitout an editor\n{message:#?}");
                    Task::none()
                }
            }
            Message::Prepare(message) => self
                .prepare
                .update(message, settings)
                .map(OrRequest::Message),
            Message::SetPaths { parent, choice } => {
                let task;
                (self.prepare, task) = prepare::Prepare::new(
                    settings,
                    parent,
                    choice,
                    Some(self.prepare.hidden()),
                    None,
                    None,
                );
                self.editor = None;
                task.map(Message::Prepare).map(OrRequest::Message)
            }
            Message::SelectDir(start) => {
                let initial_dir =
                    start.unwrap_or_else(|| settings.get::<InstallSource>().to_path_buf());
                Task::<Option<_>>::future(Self::open(initial_dir))
                    .and_then(Task::done)
                    .map(|(parent, choice)| Message::SetPaths { parent, choice })
                    .map(OrRequest::Message)
            }
            Message::SelectFile => Task::none(),
        }
    }

    /// View application state.
    pub fn view(
        &self,
        settings: &Settings,
    ) -> Element<'_, OrRequest<Message, Request>, ::iced_core::Theme, widget::Renderer> {
        if let Some(editor) = &self.editor {
            editor.view(settings)
        } else {
            self.prepare.view()
        }
    }
}
