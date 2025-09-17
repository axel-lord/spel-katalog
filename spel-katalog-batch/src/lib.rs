//! Batch command runner.

use ::iced::{
    Element, Font,
    Length::Fill,
    Task,
    widget::{
        self, button, horizontal_space,
        text_editor::{self, Action},
    },
};
use ::iced_highlighter::Highlighter;
use ::spel_katalog_common::OrRequest;

/// Message for batch view.
#[derive(Debug, Clone)]
pub enum Message {
    /// Text Editor action.
    Action(Action),
}

/// Request for batch view.
#[derive(Debug, Clone, Copy)]
pub enum Request {
    /// Request process list be shown.
    ShowProcesses,
    /// Hide batch window.
    HideBatch,
}

/// State of batch view.
#[derive(Debug)]
pub struct State {
    script: text_editor::Content,
    hl_settings: ::iced_highlighter::Settings,
}

impl Default for State {
    fn default() -> Self {
        Self {
            script: widget::text_editor::Content::with_text(include_str!("./sample.zsh")),
            hl_settings: ::iced_highlighter::Settings {
                theme: ::iced_highlighter::Theme::SolarizedDark,
                token: String::from("zsh"),
            },
        }
    }
}

impl State {
    /// Update state.
    pub fn update(&mut self, msg: Message) -> Task<OrRequest<Message, Request>> {
        match msg {
            Message::Action(action) => {
                self.script.perform(action);
                Task::none()
            }
        }
    }

    /// View widget.
    pub fn view(&self) -> Element<'_, OrRequest<Message, Request>> {
        widget::container(
            widget::Column::new()
                .push(
                    widget::Row::new().push(horizontal_space()).push(
                        button("Hide")
                            .padding(3)
                            .style(widget::button::danger)
                            .on_press_with(|| OrRequest::Request(Request::HideBatch)),
                    ).padding(3),
                )
                .push(
                    widget::text_editor(&self.script)
                        .highlight_with::<Highlighter>(self.hl_settings.clone(), |h, _| {
                            h.to_format()
                        })
                        .on_action(|act| OrRequest::Message(Message::Action(act)))
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
