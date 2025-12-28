use std::fmt;

use color_eyre::Result;
use iced::{
    Color, Element, Length, Task,
    widget::{container, space, stack},
};

use crate::{Context, Message, State, StateCommand, cmd};

pub mod confirm;
pub mod error;

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
