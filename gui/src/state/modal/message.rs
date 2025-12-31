use crate::{
    Context, TryIntoExt, bold_text,
    message::{UiMessage, ui_messages::MessageDialog as MyMessage},
};

use color_eyre::{Result, owo_colors::OwoColorize};
use iced::{
    Border, Color, Element, Length, Task,
    widget::{button, column, container, scrollable, text_editor, text_editor::Action},
};

use super::DialogResult;

#[derive(Debug, Clone)]
pub struct MessageDialog {
    pub title: String,
    editor_content: text_editor::Content,
}

impl MessageDialog {
    pub fn new(title: String, message: &str) -> Self {
        Self {
            title,
            editor_content: text_editor::Content::with_text(message),
        }
    }
}

impl super::Dialog for MessageDialog {
    fn update(&mut self, event: UiMessage, _ctx: &mut Context) -> Result<DialogResult> {
        use MyMessage::*;

        match event.try_into_ex()? {
            Confirm => Ok(DialogResult::Close(Task::none())),
            EditAction(a) => {
                if !matches!(a, Action::Edit(_)) {
                    self.editor_content.perform(a);
                }
                Ok(DialogResult::Stay)
            }
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> Element<'a, UiMessage> {
        container(
            column![
                bold_text(&self.title).size(20),
                container(
                    scrollable(
                        text_editor(&self.editor_content)
                            .on_action(|a| { MyMessage::EditAction(a).into() })
                    )
                    .height(Length::Fill)
                )
                .style(|_theme| container::background(Color::from_rgb(0.95, 0.95, 0.95)))
                .padding(20),
                container(button("Ok").on_press(MyMessage::Confirm.into()))
                    .align_right(Length::Fill)
            ]
            .spacing(10),
        )
        .height(Length::Shrink)
        .padding(20)
        .max_width(700)
        .max_height(700)
        .style(|_theme| container::background(Color::WHITE).border(Border::default().rounded(10)))
        .into()
    }
}
