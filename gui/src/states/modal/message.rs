use crate::{Context, Message};

use color_eyre::{Result, owo_colors::OwoColorize};
use iced::{
    Border, Color, Element, Font, Length, Task,
    font::Weight,
    widget::{button, column, container, scrollable, text, text_editor, text_editor::Action},
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
    fn update(&mut self, event: Message, _ctx: &mut Context) -> Result<DialogResult> {
        match event {
            Message::ErrorConfirmed => Ok(DialogResult::Close(Task::none())),
            Message::MessageModalEditAction(a) => {
                if !matches!(a, Action::Edit(_)) {
                    self.editor_content.perform(a);
                }
                Ok(DialogResult::Stay)
            }
            _ => Ok(DialogResult::Stay),
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> Element<'a, Message> {
        container(
            column![
                text(&self.title).size(20).font(Font {
                    weight: Weight::Bold,
                    ..Font::DEFAULT
                }),
                container(
                    scrollable(
                        text_editor(&self.editor_content)
                            .on_action(Message::MessageModalEditAction)
                    )
                    .height(Length::Fill)
                )
                .style(|_theme| container::background(Color::from_rgb(0.95, 0.95, 0.95)))
                .padding(20),
                container(button("Ok").on_press(Message::ErrorConfirmed)).align_right(Length::Fill)
            ]
            .spacing(10),
        )
        .height(Length::Shrink)
        .padding(20)
        .max_width(700)
        .max_height(700)
        .style(|_theme| container::background(Color::WHITE).border(Border::default().rounded(30)))
        .into()
    }
}
