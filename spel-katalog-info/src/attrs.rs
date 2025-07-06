use ::std::mem;

use ::iced::{
    Element, Task,
    widget::{self, horizontal_space},
};
use ::spel_katalog_common::w;

#[derive(Debug, Default)]
pub struct State {
    pub attrs: Vec<(String, String)>,
    key: String,
    value: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Key(String),
    Value(String),
    Add,
    Edit(usize),
}

impl State {
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

    pub fn view(&self) -> Element<'_, Message> {
        w::col()
            .push(
                w::row()
                    .push(
                        widget::text_input("Key...", &self.key)
                            .padding(3)
                            .on_input(Message::Key),
                    )
                    .push(
                        widget::text_input("Value...", &self.value)
                            .padding(3)
                            .on_input(Message::Value),
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
