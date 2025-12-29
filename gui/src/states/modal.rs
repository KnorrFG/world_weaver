use std::fmt;

use color_eyre::Result;
use iced::{
    Color, Element, Length, Task,
    widget::{container, space, stack},
};

use crate::{
    Context, Message, State, StateCommand, cmd,
    states::modal::{confirm::ConfirmDialog, edit::EditorModal, message::MessageDialog},
};

pub mod confirm;
pub mod edit;
pub mod message;

pub trait Dialog: fmt::Debug {
    fn update(&mut self, event: Message, ctx: &mut Context) -> Result<DialogResult>;
    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, Message>;
}

pub enum DialogResult {
    Stay,
    Close(Task<Message>),
}

#[derive(Debug)]
pub struct Modal<D: Dialog> {
    parent: Box<dyn State>,
    dialog: D,
}

/// Constructs a Modal wrapping an ErrorDialog
impl Modal<MessageDialog> {
    pub fn message(
        parent: Box<dyn State>,
        title: impl Into<String>,
        message: impl AsRef<str>,
    ) -> Self {
        Self::new(parent, MessageDialog::new(title.into(), message.as_ref()))
    }
}

/// Constructs a Modal wrapping a ConfirmDialog
impl Modal<ConfirmDialog> {
    pub fn confirm(
        parent: Box<dyn State>,
        message: impl Into<String>,
        yes_msg: Option<Message>,
        no_msg: Option<Message>,
    ) -> Self {
        Self::new(parent, ConfirmDialog::new(message, yes_msg, no_msg))
    }
}

/// Constructs a Modal wrapping an EditorModal
impl<F> Modal<EditorModal<F>>
where
    F: Fn(String) -> Task<Message> + Clone + Send + Sync + 'static,
{
    pub fn edit(
        parent: Box<dyn State>,
        title: impl Into<String>,
        initial_content: impl Into<String>,
        on_save: F,
    ) -> Self {
        Self::new(parent, EditorModal::new(title, initial_content, on_save))
    }
}

impl<D: Dialog> Modal<D> {
    pub fn new(parent: Box<dyn State>, dialog: D) -> Self {
        Self { parent, dialog }
    }
}

impl<D: Dialog + Clone + 'static> State for Modal<D> {
    fn update(&mut self, event: Message, ctx: &mut Context) -> Result<StateCommand> {
        match self.dialog.update(event, ctx)? {
            DialogResult::Stay => cmd::none(),
            DialogResult::Close(task) => cmd::transition_with_task(self.parent.clone(), task),
        }
    }

    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, Message> {
        stack![
            self.parent.view(ctx),
            dim_layer(),
            container(self.dialog.view(ctx)).center(Length::Fill)
        ]
        .into()
    }

    fn clone(&self) -> Box<dyn State> {
        Box::new(Self {
            parent: self.parent.clone(),
            dialog: self.dialog.clone(),
        })
    }
}

fn dim_layer() -> Element<'static, Message> {
    container(space())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style::default().background(Color::from_rgba(0., 0., 0., 0.1)))
        .into()
}
