use color_eyre::Result;
use iced::{
    Element, Length, Task,
    alignment::Horizontal,
    widget::{button, column, container, row, scrollable, text},
};

use crate::{
    context::Context,
    message::{UiMessage, ui_messages::ConfirmDialog as MyMessage},
    state::modal::{Dialog, DialogResult},
};

#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    message: String,
    yes_msg: Option<UiMessage>,
    no_msg: Option<UiMessage>,
}

impl ConfirmDialog {
    pub fn new(
        message: impl Into<String>,
        yes_msg: Option<UiMessage>,
        no_msg: Option<UiMessage>,
    ) -> Self {
        Self {
            message: message.into(),
            yes_msg,
            no_msg,
        }
    }
}

impl Dialog for ConfirmDialog {
    fn update(&mut self, event: UiMessage, _ctx: &mut Context) -> Result<DialogResult> {
        use MyMessage::*;
        if let Ok(msg) = TryInto::<MyMessage>::try_into(event) {
            match msg {
                Yes => Ok(DialogResult::Close(
                    self.yes_msg.clone().map(Task::done).unwrap_or(Task::none()),
                )),
                No => Ok(DialogResult::Close(
                    self.no_msg.clone().map(Task::done).unwrap_or(Task::none()),
                )),
            }
        } else {
            Ok(DialogResult::Stay)
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> Element<'a, UiMessage> {
        container(
            column![
                container(scrollable(text(&self.message)))
                    .padding(20)
                    .width(Length::Shrink),
                column![
                    row![
                        button("No").on_press(MyMessage::No.into()),
                        button("Yes").on_press(MyMessage::Yes.into()),
                    ]
                    .spacing(10)
                ]
                .width(Length::Fill)
                .align_x(Horizontal::Right)
                .spacing(20),
            ]
            .width(Length::Shrink)
            .spacing(20),
        )
        .padding(20)
        .style(container::bordered_box)
        .into()
    }
}
