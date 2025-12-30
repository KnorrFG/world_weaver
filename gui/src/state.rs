// mod main_menu;
// pub use main_menu::MainMenu;

use color_eyre::Result;
use iced::{Element, Task};
use std::fmt;

mod playing;

pub use playing::Playing;

pub mod modal;
pub use modal::{Dialog, Modal};

use crate::{Context, message::Message};

pub trait State: fmt::Debug {
    fn update(&mut self, event: Message, ctx: &mut Context) -> Result<StateCommand>;
    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, Message>;
    fn clone(&self) -> Box<dyn State>;
}

pub trait StateExt: State + Sized + 'static {
    fn boxed(self) -> Box<dyn State> {
        Box::new(self)
    }
}

impl<T: State + Sized + 'static> StateExt for T {}

impl<T: std::ops::DerefMut<Target = dyn State> + fmt::Debug> State for T {
    fn update(&mut self, event: Message, ctx: &mut Context) -> Result<StateCommand> {
        self.deref_mut().update(event, ctx)
    }

    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, Message> {
        self.deref().view(ctx)
    }

    fn clone(&self) -> Box<dyn State> {
        self.deref().clone()
    }
}

#[derive(Debug, Default)]
pub struct StateCommand {
    pub task: Option<Task<Message>>,
    pub transition: Option<Box<dyn State>>,
}

pub mod cmd {
    use super::*;

    pub fn none() -> Result<StateCommand> {
        Ok(StateCommand::default())
    }

    pub fn task(t: Task<Message>) -> Result<StateCommand> {
        Ok(StateCommand {
            task: Some(t),
            transition: None,
        })
    }

    pub fn transition(s: impl State + 'static) -> Result<StateCommand> {
        Ok(StateCommand {
            task: None,
            transition: Some(Box::new(s)),
        })
    }

    pub fn transition_with_task(s: impl State + 'static, t: Task<Message>) -> Result<StateCommand> {
        Ok(StateCommand {
            task: Some(t),
            transition: Some(Box::new(s)),
        })
    }
}
