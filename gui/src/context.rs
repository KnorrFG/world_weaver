use color_eyre::{Result, eyre::ensure};
use engine::{game::Game, save_archive::SaveArchive};
use iced::Task;
use log::debug;

use crate::{
    Config, active_game_save_path,
    context::game_context::GameContext,
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

    pub fn load_game(&mut self) -> Result<&Game> {
        self.game = None;
        let save_path = active_game_save_path()?;
        ensure!(
            save_path.exists(),
            "No game running. Please start a new one via the NewGame command"
        );

        debug!("Loading save: {save_path:?}");
        let mut archive = SaveArchive::open(save_path)?;
        let game_data = archive.read_game_data()?;
        let game = Game::load(
            self.config.get_llm(),
            self.config.get_image_model()?,
            game_data,
        );
        self.game = Some(GameContext::try_new(game, archive)?);
        Ok(&self.game.as_ref().unwrap().game)
    }
}
