use crate::{
    context::Context,
    message::{UiMessage, ui_messages::EditDialog as MyMessage},
    state::{
        Dialog,
        modal::DialogResult,
    },
};
use color_eyre::Result;
use iced::{
    Border, Color, Element, Length, Task, padding,
    widget::{button, column, container, row, space, text, text_editor},
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
    F: Fn(String) -> Task<UiMessage> + Clone + Send + Sync + 'static,
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
    F: Fn(String) -> Task<UiMessage> + Clone + Send + Sync + 'static,
{
    fn update(&mut self, event: UiMessage, _ctx: &mut Context) -> Result<DialogResult> {
        use MyMessage::*;
        if let Ok(msg) = TryInto::<MyMessage>::try_into(event) {
            match msg {
                Update(action) => {
                    self.editor_content.perform(action);
                    Ok(DialogResult::Stay)
                }
                Save => {
                    let task = (self.on_save)(self.editor_content.text());
                    Ok(DialogResult::Close(task))
                }
                Cancel => Ok(DialogResult::Close(Task::none())),
            }
        } else {
            Ok(DialogResult::Stay)
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> Element<'a, UiMessage> {
        let editor = text_editor(&self.editor_content).on_action(|a| MyMessage::Update(a).into());

        let content = container(container(editor).padding(padding::all(10).right(20)))
            .height(Length::Shrink)
            .max_height(500)
            .style(|_theme| container::background(Color::from_rgb(0.95, 0.95, 0.95)));

        container(
            column![
                text(&self.title).size(20),
                content,
                row![
                    space::horizontal(),
                    button("Cancel").on_press(MyMessage::Cancel.into()),
                    button("Save").on_press(MyMessage::Save.into()),
                ]
                .spacing(10)
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
