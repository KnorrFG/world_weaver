use color_eyre::Result;
use iced::{
    Border, Element, Length, Task,
    widget::{button, column, container, row, scrollable, text},
};

use crate::{
    Context, Message,
    states::modal::{Dialog, DialogResult},
};

#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    message: String,
    yes_msg: Option<Message>,
    no_msg: Option<Message>,
}

impl ConfirmDialog {
    pub fn new(
        message: impl Into<String>,
        yes_msg: Option<Message>,
        no_msg: Option<Message>,
    ) -> Self {
        Self {
            message: message.into(),
            yes_msg,
            no_msg,
        }
    }
}

impl Dialog for ConfirmDialog {
    fn update(&mut self, event: Message, _ctx: &mut Context) -> Result<DialogResult> {
        match event {
            Message::ConfirmDialogYes => Ok(DialogResult::Close(
                self.yes_msg.clone().map(Task::done).unwrap_or(Task::none()),
            )),
            Message::ConfirmDialogNo => Ok(DialogResult::Close(
                self.no_msg.clone().map(Task::done).unwrap_or(Task::none()),
            )),
            _ => Ok(DialogResult::Stay),
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> Element<'a, Message> {
        container(
            column![
                container(scrollable(text(&self.message)))
                    .padding(20)
                    .width(Length::Shrink),
                row![
                    button("No").on_press(Message::ConfirmDialogNo),
                    button("Yes").on_press(Message::ConfirmDialogYes),
                ]
                .spacing(20),
            ]
            .spacing(20),
        )
        .padding(20)
        .style(|theme| iced::widget::container::secondary(theme).border(Border::default()))
        .into()
    }
}
