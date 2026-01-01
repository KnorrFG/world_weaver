use crate::{
    TryIntoExt,
    context::Context,
    message::{UiMessage, ui_messages::InputDialog as MyMessage},
    state::{Dialog, modal::DialogResult},
};
use color_eyre::Result;
use iced::{
    Border, Color, Element, Length, Task,
    widget::{button, column, container, row, scrollable, space, text, text_editor, text_input},
};

/// A generic editor modal that produces a Task<Message> when saved
#[derive(Clone)]
pub struct InputDialog<F> {
    title: String,
    input: String,
    placeholder: String,
    on_save: F,
}

impl<F> std::fmt::Debug for InputDialog<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputModal")
            .field("title", &self.title)
            .field("input", &self.input)
            .field("on_save", &"...")
            .finish()
    }
}

impl<F> InputDialog<F>
where
    F: Fn(String) -> Task<UiMessage> + Clone + Send + Sync + 'static,
{
    pub fn new(title: impl Into<String>, placeholder: impl Into<String>, on_save: F) -> Self {
        Self {
            title: title.into(),
            input: String::new(),
            placeholder: placeholder.into(),
            on_save,
        }
    }
}

impl<F> Dialog for InputDialog<F>
where
    F: Fn(String) -> Task<UiMessage> + Clone + Send + Sync + 'static,
{
    fn update(&mut self, event: UiMessage, _ctx: &mut Context) -> Result<DialogResult> {
        use MyMessage::*;
        match event.try_into_ex()? {
            Edit(content) => {
                self.input = content;
                Ok(DialogResult::Stay)
            }
            Save => {
                let task = (self.on_save)(self.input.clone());
                Ok(DialogResult::Close(task))
            }
            Cancel => Ok(DialogResult::Close(Task::none())),
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> Element<'a, UiMessage> {
        let content = column![
            text(&self.title).size(20),
            text_input(&self.placeholder, &self.input)
                .on_submit(MyMessage::Save.into())
                .on_input(|a| MyMessage::Edit(a).into()),
            row![
                space::horizontal(),
                button("Cancel").on_press(MyMessage::Cancel.into()),
                button("Ok").on_press(MyMessage::Save.into()),
            ]
            .spacing(10)
        ]
        .spacing(10)
        .padding(20);

        container(
            container(content)
                .style(|_theme| {
                    container::background(Color::WHITE).border(Border::default().rounded(10))
                })
                .max_width(700)
                .max_height(700),
        )
        .center(Length::Fill)
        .into()
    }
}
