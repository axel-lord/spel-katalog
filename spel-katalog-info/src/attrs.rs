//! Attribute editor implementation.

use ::core::mem;

use ::iced_runtime::Task;
use ::iced_widget::{self as widget, horizontal_space};
use ::spel_katalog_common::w;

use crate::Element;

/// State of attribute editor.
#[derive(Debug, Default)]
pub struct State {
    /// Attributes of game.
    pub attrs: Vec<(String, String)>,
    /// Current key.
    key: String,
    /// Current value.
    value: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Current key should be set to value.
    Key(String),
    /// Current value should be set to value.
    Value(String),
    /// Current field contents as an attribute.
    Add,
    /// Move the attribute at the given index to fields for editing.
    Edit(usize),
}

impl State {
    /// Update state of attribute editor according to the given message.
    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Key(key) => self.key = key,
            Message::Value(value) => self.value = value,
            Message::Add => {
                let key = mem::take(&mut self.key);
                let value = mem::take(&mut self.value);
                self.attrs.push((key, value));
            }
            Message::Edit(idx) => {
                if idx < self.attrs.len() {
                    let (key, value) = self.attrs.remove(idx);
                    self.key = key;
                    self.value = value;
                }
            }
        }
        Task::none()
    }

    /// View the attribute editor.
    pub fn view(&self) -> Element<'_, Message> {
        w::col()
            .push(
                w::row()
                    .push(
                        widget::text_input("Key...", &self.key)
                            .padding(3)
                            .on_input(Message::Key)
                            .on_submit(Message::Add),
                    )
                    .push(
                        widget::text_input("Value...", &self.value)
                            .padding(3)
                            .on_input(Message::Value)
                            .on_submit(Message::Add),
                    )
                    .push(
                        widget::button("Add")
                            .padding(3)
                            .on_press_with(|| Message::Add),
                    ),
            )
            .extend(self.attrs.iter().enumerate().map(|(i, (key, value))| {
                w::row()
                    .push(key.as_str())
                    .push("=")
                    .push(value.as_str())
                    .push(horizontal_space())
                    .push(
                        widget::button("Edit")
                            .padding(3)
                            .on_press_with(move || Message::Edit(i)),
                    )
                    .into()
            }))
            .into()
    }
}
