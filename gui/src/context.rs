use color_eyre::Result;
use iced::Task;

use crate::{
    Config,
    message::{ContextMessage, Message},
};

pub mod game_context;

pub struct Context {
    pub game: Option<game_context::GameContext>,
    pub config: Config,
}

impl Context {
    pub fn from_config(config: Config) -> Self {
        Self { game: None, config }
    }

    pub fn update(&mut self, message: ContextMessage) -> Result<Task<Message>> {
        if let Some(gc) = &mut self.game {
            gc.update(message)
        } else {
            Ok(Task::none())
        }
    }
}
