use ::std::sync::Arc;

use ::iced::{
    Element,
    Length::Fill,
    Task,
    widget::{Column, Row, button, container, horizontal_rule, horizontal_space, scrollable},
};
use ::tap::Pipe;
use ::tokio::sync::mpsc::{Receiver, Sender, channel};

#[derive(Debug, Clone)]
pub struct Dialog {
    sender: Sender<String>,
    buttons: Arc<[String]>,
    multiline: bool,
    text: String,
}

/// The given string was clicked.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Message {
    Clicked(String),
}

/// Request to be closed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Request {
    #[default]
    Close,
}

fn btn_style(theme: &::iced::Theme, status: button::Status, name: &str) -> button::Style {
    match name {
        "Ok" | "ok" | "Yes" | "yes" | "Y" | "y" => button::success(theme, status),
        "Cancel" | "cancel" | "Exit" | "exit" | "No" | "no" | "N" | "n" => {
            button::danger(theme, status)
        }
        _ => button::primary(theme, status),
    }
}

impl Dialog {
    /// Construct a new dialog, and get a receiver for events.
    pub fn new(
        text: impl Into<String>,
        buttons: impl IntoIterator<Item = impl Into<String>>,
    ) -> (Dialog, Receiver<String>) {
        let (sender, rx) = channel(64);
        let text = text.into();
        let buttons = buttons.into_iter().map(Into::into).collect();
        let multiline = text.contains('\n');
        (
            Self {
                buttons,
                text,
                multiline,
                sender,
            },
            rx,
        )
    }

    pub fn update(&mut self, msg: Message) -> Task<Request> {
        match msg {
            Message::Clicked(button) => {
                let sender = self.sender.clone();

                Task::future(async move {
                    if let Err(err) = sender.send(button).await {
                        ::log::error!("failed to send button\n{err}");
                    };

                    Request::Close
                })
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        Column::new()
            .padding(3)
            .spacing(3)
            .push(if self.multiline {
                scrollable(self.text.as_str())
                    .height(Fill)
                    .width(Fill)
                    .pipe(Element::from)
            } else {
                container(self.text.as_str())
                    .center(Fill)
                    .pipe(Element::from)
            })
            .push(horizontal_rule(3))
            .push(Row::new().spacing(3).push(horizontal_space()).extend(
                self.buttons.iter().rev().map(|label| {
                    button(label.as_str())
                        .style(|t, s| btn_style(t, s, label))
                        .padding(3)
                        .on_press_with(|| Message::Clicked(label.clone()))
                        .pipe(Element::from)
                }),
            ))
            .into()
    }
}
