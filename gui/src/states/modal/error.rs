use crate::{Context, Message};

use color_eyre::Result;
use iced::{
    Border, Element, Length, Task,
    widget::{button, column, container, scrollable, text},
};

use super::DialogResult;

#[derive(Debug, Clone)]
pub struct ErrorDialog {
    pub message: String,
}

impl ErrorDialog {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl super::Dialog for ErrorDialog {
    fn update(&mut self, event: Message, _ctx: &mut Context) -> Result<DialogResult> {
        match event {
            Message::ErrorConfirmed => Ok(DialogResult::Close(Task::none())),
            _ => Ok(DialogResult::Stay),
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> Element<'a, Message> {
        container(
            column![
                text("Error:"),
                container(scrollable(text(&self.message))).padding(20),
                container(button("Ok").on_press(Message::ErrorConfirmed)).align_right(Length::Fill)
            ]
            .width(Length::Shrink)
            .spacing(10),
        )
        .padding(20)
        .style(|theme| container::secondary(theme).border(Border::default()))
        .into()
    }
}
