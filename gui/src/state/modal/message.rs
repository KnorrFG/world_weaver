use crate::{
    TryIntoExt, bold_text,
    context::Context,
    message::{UiMessage, ui_messages::MessageDialog as MyMessage},
    state::modal::modal_outer_container,
};

use color_eyre::Result;
use iced::{
    Element, Length, Task, padding,
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
        modal_outer_container(
            column![
                bold_text(&self.title).size(20),
                scrollable(
                    container(
                        text_editor(&self.editor_content)
                            .on_action(|a| { MyMessage::EditAction(a).into() })
                    )
                    .padding(padding::all(10).right(20))
                )
                .height(Length::Fill),
                container(button("Ok").on_press(MyMessage::Confirm.into()))
                    .align_right(Length::Fill)
            ]
            .spacing(10),
        )
        .into()
    }
}
