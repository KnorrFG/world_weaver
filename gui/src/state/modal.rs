use std::fmt;

use color_eyre::Result;
use iced::{
    Border, Color, Element, Length, Task,
    widget::{Container, container, scrollable, space, stack},
};

use crate::{
    State,
    context::Context,
    message::UiMessage,
    state::{
        StateCommand, cmd,
        modal::{
            confirm::ConfirmDialog, edit::EditorModal, input::InputDialog, message::MessageDialog,
        },
    },
};

pub mod confirm;
pub mod edit;
pub mod input;
pub mod message;

pub trait Dialog: fmt::Debug {
    fn update(&mut self, event: UiMessage, ctx: &mut Context) -> Result<DialogResult>;
    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, UiMessage>;
}

pub enum DialogResult {
    Stay,
    Close(Task<UiMessage>),
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
        yes_msg: Option<UiMessage>,
        no_msg: Option<UiMessage>,
    ) -> Self {
        Self::new(parent, ConfirmDialog::new(message, yes_msg, no_msg))
    }
}

/// Constructs a Modal wrapping an InputDialog
impl<F> Modal<InputDialog<F>>
where
    F: Fn(String) -> Task<UiMessage> + Clone + Send + Sync + 'static,
{
    pub fn input(
        parent: Box<dyn State>,
        title: impl Into<String>,
        placeholder: impl Into<String>,
        ok_msg: F,
    ) -> Self {
        Self::new(parent, InputDialog::new(title, placeholder, ok_msg))
    }
}

/// Constructs a Modal wrapping an EditorModal
impl<F> Modal<EditorModal<F>>
where
    F: Fn(String) -> Task<UiMessage> + Clone + Send + Sync + 'static,
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
    fn update(&mut self, event: UiMessage, ctx: &mut Context) -> Result<StateCommand> {
        match self.dialog.update(event, ctx)? {
            DialogResult::Stay => cmd::none(),
            DialogResult::Close(task) => cmd::transition_with_task(self.parent.clone(), task),
        }
    }

    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, UiMessage> {
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

fn dim_layer() -> Element<'static, UiMessage> {
    container(space())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style::default().background(Color::from_rgba(0., 0., 0., 0.1)))
        .into()
}

fn modal_outer_container<'a>(child: impl Into<Element<'a, UiMessage>>) -> Container<'a, UiMessage> {
    container(scrollable(child))
        .height(Length::Shrink)
        .padding(20)
        .max_width(700)
        .max_height(700)
        .style(|_theme| container::background(Color::WHITE).border(Border::default().rounded(10)))
}
