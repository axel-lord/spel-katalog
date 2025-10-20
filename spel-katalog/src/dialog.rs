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
pub struct DialogBuilder {
    sender: Sender<String>,
    buttons: Arc<[String]>,
    text: Arc<str>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum ButtonTheme {
    #[default]
    Primary,
    Danger,
    Success,
}

impl ButtonTheme {
    fn from_name(name: &str) -> Self {
        match name {
            "Ok" | "ok" | "Yes" | "yes" | "Y" | "y" => Self::Success,
            "Cancel" | "cancel" | "Exit" | "exit" | "No" | "no" | "N" | "n" => Self::Danger,
            _ => Self::Primary,
        }
    }

    fn style(self, theme: &::iced::Theme, status: button::Status) -> button::Style {
        match self {
            ButtonTheme::Primary => button::primary(theme, status),
            ButtonTheme::Danger => button::danger(theme, status),
            ButtonTheme::Success => button::success(theme, status),
        }
    }
}

impl DialogBuilder {
    /// Construct a new dialog, and get a receiver for events.
    pub fn new(
        text: impl AsRef<str>,
        buttons: impl IntoIterator<Item = impl Into<String>>,
    ) -> (DialogBuilder, Receiver<String>) {
        let (sender, rx) = channel(64);
        let text = Arc::<str>::from(text.as_ref());
        let buttons = buttons.into_iter().map(Into::into).collect();
        (
            Self {
                buttons,
                text,
                sender,
            },
            rx,
        )
    }

    pub fn build(self) -> Dialog {
        let Self {
            sender,
            buttons,
            text,
        } = self;
        let text = text.to_string();
        let multiline = text.contains('\n');
        let buttons = buttons
            .into_iter()
            .map(|name| (name.clone(), ButtonTheme::from_name(name)))
            .collect();
        Dialog {
            multiline,
            sender,
            buttons,
            text,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Dialog {
    multiline: bool,
    sender: Sender<String>,
    buttons: Box<[(String, ButtonTheme)]>,
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

impl Dialog {
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
                self.buttons.iter().rev().map(|(label, style)| {
                    button(label.as_str())
                        .style(|t, s| style.style(t, s))
                        .padding(3)
                        .on_press_with(|| Message::Clicked(label.clone()))
                        .pipe(Element::from)
                }),
            ))
            .into()
    }
}
