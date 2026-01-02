mod main_menu;
pub use main_menu::MainMenu;

use color_eyre::Result;
use iced::{Element, Task};
use std::fmt;

mod playing;
pub use playing::Playing;

pub mod modal;
pub use modal::{Dialog, Modal};

pub mod world_menu;
pub use world_menu::WorldMenu;

pub mod world_editor;
pub use world_editor::WorldEditor;

pub mod load_menu;
pub mod options_menu;
pub mod start_new_game;

use crate::{
    context::Context,
    message::{Message, UiMessage},
};

pub trait State: fmt::Debug {
    fn update(&mut self, event: UiMessage, ctx: &mut Context) -> Result<StateCommand>;
    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, UiMessage>;
    fn clone(&self) -> Box<dyn State>;
}

pub trait StateExt: State + Sized + 'static {
    fn boxed(self) -> Box<dyn State> {
        Box::new(self)
    }
}

impl<T: State + Sized + 'static> StateExt for T {}

impl<T: std::ops::DerefMut<Target = dyn State> + fmt::Debug> State for T {
    fn update(&mut self, event: UiMessage, ctx: &mut Context) -> Result<StateCommand> {
        self.deref_mut().update(event, ctx)
    }

    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, UiMessage> {
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
    use iced::advanced::graphics::futures::MaybeSend;

    use super::*;

    pub fn none() -> Result<StateCommand> {
        Ok(StateCommand::default())
    }

    pub fn task<T>(t: Task<T>) -> Result<StateCommand>
    where
        T: Into<Message> + 'static,
        T: MaybeSend,
    {
        Ok(StateCommand {
            task: Some(t.map(|x| x.into())),
            transition: None,
        })
    }

    pub fn transition(s: impl State + 'static) -> Result<StateCommand> {
        Ok(StateCommand {
            task: None,
            transition: Some(Box::new(s)),
        })
    }

    pub fn transition_with_task<T>(s: impl State + 'static, t: Task<T>) -> Result<StateCommand>
    where
        T: Into<Message> + 'static,
        T: MaybeSend,
    {
        Ok(StateCommand {
            task: Some(t.map(|x| x.into())),
            transition: Some(Box::new(s)),
        })
    }
}
