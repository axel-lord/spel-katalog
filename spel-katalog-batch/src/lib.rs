//! Batch command runner.

use ::std::{
    io::{Write, pipe},
    process::Command,
    sync::Arc,
};

use ::derive_more::Display;
use ::iced::{
    Alignment::Center,
    Element, Font,
    Length::Fill,
    Task,
    keyboard::{Key, Modifiers},
    widget::{
        self, button, text,
        text_editor::{self, Action, Binding, Edit},
    },
};
use ::iced_highlighter::Highlighter;
use ::serde::Serialize;
use ::spel_katalog_common::{OrRequest, StatusSender, async_status};
use ::strum::VariantArray;
use ::tap::Pipe;

/// One entry to be sent to batch script.
#[derive(Debug, Clone, Default, Serialize)]
pub struct BatchInfo {
    /// Numeric id of game.
    pub id: i64,
    /// Game slug.
    pub slug: String,
    /// Game name.
    pub name: String,
    /// Game runner.
    pub runner: String,
    /// Path to yml config for game.
    pub config: String,
    /// True if the game is hidden.
    pub hidden: bool,
}

/// Message for batch view.
#[derive(Debug, Clone)]
pub enum Message {
    /// Text Editor action.
    Action(Action),
    /// Set language in use.
    Lang(Language),
    /// Set scope in use.
    Scope(Scope),
    /// Run batch script on info.
    RunBatch(Vec<BatchInfo>),
    /// Set script content.
    SetContent(String, String, Language),
    /// Clear batch script.
    Clear,
    /// Open batch script.
    Open,
    /// Save batch script.
    Save,
    /// Insert four spaces.
    Indent,
}

/// Request for batch view.
#[derive(Debug, Clone, Copy)]
pub enum Request {
    /// Request process list be shown.
    ShowProcesses,
    /// Hide batch window.
    HideBatch,
    /// Gather batch info
    GatherBatchInfo(Scope),
}

/// State of batch view.
#[derive(Debug)]
pub struct State {
    script: text_editor::Content,
    hl_settings: ::iced_highlighter::Settings,
    lang: Language,
    scope: Scope,
    script_title: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            script: widget::text_editor::Content::with_text(include_str!("./sample.zsh")),
            hl_settings: ::iced_highlighter::Settings {
                theme: ::iced_highlighter::Theme::SolarizedDark,
                token: String::from("zsh"),
            },
            lang: Language::Zsh,
            scope: Scope::default(),
            script_title: "samle.zsh".to_owned(),
        }
    }
}

/// Language to execute using.
#[derive(Debug, Clone, Copy, Default, VariantArray, Display, PartialEq, Eq)]
pub enum Language {
    /// Execute as a zsh script.
    #[display("zsh")]
    Zsh,
    /// Execute as a bash script.
    #[default]
    #[display("bash")]
    Bash,
    /// Execute as a python script.
    #[display("python")]
    Python,
}

/// What games to use as input.
#[derive(Debug, Clone, Copy, Default, VariantArray, Display, PartialEq, Eq)]
pub enum Scope {
    /// Use all games.
    All,
    /// Use currently shown games.
    #[default]
    Shown,
    /// Use currently batch selected games.
    Batch,
}

impl State {
    /// Update state.
    pub fn update(
        &mut self,
        msg: Message,
        tx: &StatusSender,
        settings: &::spel_katalog_settings::Settings,
    ) -> Task<OrRequest<Message, Request>> {
        match msg {
            Message::Action(action) => {
                self.script.perform(action);
                Task::none()
            }
            Message::Lang(language) => {
                self.hl_settings.token = language.to_string();
                self.lang = language;
                Task::none()
            }
            Message::RunBatch(batch_infos) => {
                let lang = self.lang;
                let script = self.script.text();
                let title = Some(self.script_title.clone()).filter(|s| !s.is_empty());
                let task = Task::future(::tokio::task::spawn_blocking(move || {
                    let (r, mut w) = pipe()?;
                    let mut command = match lang {
                        Language::Zsh => {
                            let mut command = Command::new("zsh");
                            command
                                .args(["--emulate", "zsh", "-c"])
                                .arg(script)
                                .arg(title.unwrap_or_else(|| "batch-script-zsh".to_owned()));
                            command
                        }
                        Language::Bash => {
                            let mut command = Command::new("bash");
                            command
                                .arg("-c")
                                .arg(script)
                                .arg(title.unwrap_or_else(|| "batch-script-bash".to_owned()));
                            command
                        }
                        Language::Python => {
                            let mut command = Command::new("python3");
                            command.arg("-c").arg(script);
                            command
                        }
                    };

                    let mut child = command.stdin(r).spawn()?;

                    for info in batch_infos {
                        ::serde_json::to_writer(&mut w, &info)
                            .map_err(|err| ::std::io::Error::other(err))?;
                        w.write_all(b"\n")?;
                    }

                    w.flush()?;
                    drop(w);

                    let status = child.wait()?;

                    ::log::info!("batch script finished, {status}");

                    Ok::<_, ::std::io::Error>(())
                }))
                .then(|result| match result {
                    Ok(Ok(..)) => Task::none(),
                    Ok(Err(err)) => {
                        ::log::error!("Failure when running batch\n{err}");
                        Task::none()
                    }
                    Err(err) => {
                        ::log::error!("Could not spawn blocking task\n{err}");
                        Task::none()
                    }
                });

                Task::batch([task, Task::done(OrRequest::Request(Request::ShowProcesses))])
            }
            Message::Clear => {
                self.script_title.clear();
                [Action::SelectAll, Action::Edit(Edit::Backspace)]
                    .into_iter()
                    .for_each(|action| self.script.perform(action));
                Task::none()
            }
            Message::Open => {
                let tx = tx.clone();
                let batch_dir = settings
                    .get::<::spel_katalog_settings::BatchScriptDir>()
                    .as_path()
                    .to_path_buf();
                Task::future(async move {
                    let file_path = ::rfd::AsyncFileDialog::new()
                        .set_title("Save Batch Script")
                        .set_directory(batch_dir)
                        .pick_file()
                        .await;
                    let Some(file_path) = file_path else {
                        ::log::info!("batch script open cancelled");
                        return None;
                    };
                    let file_path = file_path.path();
                    let result = ::tokio::fs::read_to_string(file_path).await;

                    match result {
                        Err(err) => {
                            ::log::error!("could not open script {file_path:?}\n{err}");
                            async_status!(tx, "could not open script {file_path:?}").await;
                            None
                        }
                        Ok(content) => {
                            let ext = file_path
                                .extension()
                                .and_then(|ext| ext.to_str())
                                .unwrap_or_default();
                            let ext = if ext.eq_ignore_ascii_case("zsh") {
                                Language::Zsh
                            } else if ext.eq_ignore_ascii_case("py") {
                                Language::Python
                            } else {
                                Language::Bash
                            };

                            Some(OrRequest::Message(Message::SetContent(
                                content,
                                file_path
                                    .file_name()
                                    .map(|name| name.display().to_string())
                                    .unwrap_or_default(),
                                ext,
                            )))
                        }
                    }
                })
                .then(|result| match result {
                    Some(msg) => Task::done(msg),
                    None => Task::none(),
                })
            }
            Message::Save => {
                let content = self.script.text();
                let tx = tx.clone();
                let batch_dir = settings
                    .get::<::spel_katalog_settings::BatchScriptDir>()
                    .as_path()
                    .to_path_buf();
                Task::future(async move {
                    let file_path = ::rfd::AsyncFileDialog::new()
                        .set_title("Save Batch Script")
                        .set_directory(batch_dir)
                        .save_file()
                        .await;
                    let Some(file_path) = file_path else {
                        ::log::info!("batch script save cancelled");
                        return;
                    };
                    let file_path = file_path.path();
                    let result = ::tokio::fs::write(file_path, content.as_bytes()).await;

                    if let Err(err) = result {
                        ::log::error!("could not save script to {file_path:?}\n{err}");
                        async_status!(tx, "could not save script to {file_path:?}").await;
                    }
                })
                .then(|_| Task::none())
            }
            Message::Indent => {
                for _ in 0..4 {
                    self.script.perform(Action::Edit(Edit::Insert(' ')));
                }
                Task::none()
            }
            Message::SetContent(content, title, language) => {
                self.script_title = title;
                self.lang = language;
                self.hl_settings.token = language.to_string();
                [
                    Action::SelectAll,
                    Action::Edit(Edit::Backspace),
                    Action::Edit(Edit::Paste(Arc::new(content))),
                ]
                .into_iter()
                .for_each(|action| self.script.perform(action));
                Task::none()
            }
            Message::Scope(scope) => {
                self.scope = scope;
                Task::none()
            }
        }
    }

    /// View widget.
    pub fn view(&self) -> Element<'_, OrRequest<Message, Request>> {
        widget::container(
            widget::Column::new()
                .push(
                    widget::Row::new()
                        .align_y(Center)
                        .push(
                            widget::pick_list(Language::VARIANTS, Some(self.lang), |l| {
                                OrRequest::Message(Message::Lang(l))
                            })
                            .padding(3),
                        )
                        .push(text(&self.script_title).center().width(Fill))
                        .push(
                            widget::pick_list(Scope::VARIANTS, Some(self.scope), |s| {
                                OrRequest::Message(Message::Scope(s))
                            })
                            .padding(3),
                        )
                        .push(
                            button("Run")
                                .padding(3)
                                .style(widget::button::success)
                                .on_press_with(|| {
                                    OrRequest::Request(Request::GatherBatchInfo(self.scope))
                                }),
                        )
                        .push(
                            button("Clear")
                                .padding(3)
                                .style(widget::button::danger)
                                .on_press(OrRequest::Message(Message::Clear)),
                        )
                        .push(
                            button("Open")
                                .padding(3)
                                .on_press(OrRequest::Message(Message::Open)),
                        )
                        .push(
                            button("Save")
                                .padding(3)
                                .on_press(OrRequest::Message(Message::Save)),
                        )
                        .push(
                            button("Hide")
                                .padding(3)
                                .style(widget::button::danger)
                                .on_press_with(|| OrRequest::Request(Request::HideBatch)),
                        )
                        .padding(3)
                        .spacing(3),
                )
                .push(
                    widget::text_editor(&self.script)
                        .highlight_with::<Highlighter>(self.hl_settings.clone(), |h, _| {
                            h.to_format()
                        })
                        .on_action(|act| OrRequest::Message(Message::Action(act)))
                        .key_binding(|keypress| {
                            if keypress.key.as_ref()
                                == Key::Named(::iced::keyboard::key::Named::Tab)
                            {
                                Some(Binding::Custom(OrRequest::Message(Message::Indent)))
                            } else if keypress.key.as_ref() == Key::Character("r")
                                && keypress.modifiers == Modifiers::CTRL
                            {
                                Some(Binding::Custom(OrRequest::Request(
                                    Request::GatherBatchInfo(self.scope),
                                )))
                            } else if keypress.modifiers == Modifiers::CTRL
                                && keypress.key.as_ref() == Key::Character("d")
                            {
                                (1..=10)
                                    .map(|id| BatchInfo {
                                        id,
                                        slug: format!("game-{id}"),
                                        name: format!("Game {id}"),
                                        runner: "wine".to_owned(),
                                        config: "/dev/null".to_owned(),
                                        hidden: false,
                                    })
                                    .collect::<Vec<_>>()
                                    .pipe(Message::RunBatch)
                                    .pipe(OrRequest::Message)
                                    .pipe(Binding::Custom)
                                    .pipe(Some)
                            } else {
                                Binding::from_key_press(keypress)
                            }
                        })
                        .font(Font::MONOSPACE)
                        .height(Fill),
                )
                .height(Fill),
        )
        .style(|theme| {
            ::spel_katalog_common::styling::box_border(theme).background(theme.palette().background)
        })
        .max_width(800)
        .height(Fill)
        .into()
    }
}
