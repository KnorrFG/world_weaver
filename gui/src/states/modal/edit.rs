use crate::{
    Context, Message,
    states::{Dialog, modal::DialogResult},
};
use color_eyre::Result;
use iced::{
    Element, Length, Task, Theme,
    widget::{button, column, container, row, scrollable, space, text, text_editor},
};

/// A generic editor modal that produces a Task<Message> when saved
#[derive(Clone)]
pub struct EditorModal<F> {
    title: String,
    editor_content: text_editor::Content,
    on_save: F,
}

impl<F> std::fmt::Debug for EditorModal<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorModal")
            .field("title", &self.title)
            .field("editor_content", &self.editor_content)
            .field("on_save", &"...")
            .finish()
    }
}

impl<F> EditorModal<F>
where
    F: Fn(String) -> Task<Message> + Clone + Send + Sync + 'static,
{
    pub fn new(title: impl Into<String>, initial_content: impl Into<String>, on_save: F) -> Self {
        Self {
            title: title.into(),
            editor_content: text_editor::Content::with_text(&initial_content.into()),
            on_save,
        }
    }
}

impl<F> Dialog for EditorModal<F>
where
    F: Fn(String) -> Task<Message> + Clone + Send + Sync + 'static,
{
    fn update(&mut self, event: Message, _ctx: &mut Context) -> Result<DialogResult> {
        match event {
            Message::UpdateEditModal(action) => {
                self.editor_content.perform(action);
                Ok(DialogResult::Stay)
            }
            Message::SaveEditModal => {
                let task = (self.on_save)(self.editor_content.text());
                Ok(DialogResult::Close(task))
            }
            Message::CancelEditModal => Ok(DialogResult::Close(Task::none())),
            _ => Ok(DialogResult::Stay),
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> Element<'a, Message> {
        let editor = text_editor(&self.editor_content).on_action(Message::UpdateEditModal);

        let content = column![
            text(&self.title).size(20),
            scrollable(editor).height(Length::Fill),
            row![
                space::horizontal(),
                button("Cancel").on_press(Message::CancelEditModal),
                button("Save").on_press(Message::SaveEditModal),
            ]
            .spacing(10)
        ]
        .spacing(10)
        .padding(20);

        container(
            container(content)
                .style(container::rounded_box)
                .max_width(700)
                .max_height(700),
        )
        .center(Length::Fill)
        .into()
    }
}
