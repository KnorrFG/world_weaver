use std::collections::BTreeMap;

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use engine::{
    ImgModBox, LLMBox,
    game::Game,
    image_model::{self, Model, ModelStyle},
    llm::{self},
    save_archive::SaveArchive,
};
use iced::Task;
use log::debug;
use serde::{Deserialize, Serialize};

use crate::{
    active_game_save_path,
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
            self.config.get_llm()?,
            self.config.get_image_model()?,
            game_data,
            self.config.active_style().cloned(),
        );
        self.game = Some(GameContext::try_new(game, archive)?);
        Ok(&self.game.as_ref().unwrap().game)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    pub current_img_model: image_model::ProvidedModel,
    pub current_llm: llm::ProvidedModel,
    pub img_model_tokens: BTreeMap<image_model::ModelProvider, String>,
    pub llm_tokens: BTreeMap<llm::ModelProvider, String>,
    pub active_model_style: BTreeMap<image_model::Model, String>,
    pub styles: BTreeMap<StyleKey, ModelStyle>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StyleKey {
    pub model: image_model::Model,
    pub name: String,
}

impl Config {
    pub fn get_llm(&self) -> Result<LLMBox> {
        let model = self.current_llm;
        let key = self
            .llm_tokens
            .get(&model.provider())
            .ok_or(eyre!("No token for {model:?}"))?;
        Ok(model.make(key.clone()))
    }

    pub fn get_image_model(&self) -> Result<ImgModBox> {
        let model = self.current_img_model;
        let key = self
            .img_model_tokens
            .get(&model.provider())
            .ok_or(eyre!("No token for {model}"))?;
        Ok(model.make(key.clone()))
    }

    pub fn active_style_for_mut(&mut self, model: Model) -> Option<&mut image_model::ModelStyle> {
        let name = self.active_model_style.get(&model)?;
        self.styles.get_mut(&StyleKey {
            model,
            name: name.clone(),
        })
    }

    pub fn active_style_for(&self, model: Model) -> Option<&image_model::ModelStyle> {
        let name = self.active_model_style.get(&model)?;
        self.styles.get(&StyleKey {
            model,
            name: name.clone(),
        })
    }

    pub fn active_style(&self) -> Option<&image_model::ModelStyle> {
        let model = self.current_img_model.model();
        let name = self.active_model_style.get(&model)?;
        self.styles.get(&StyleKey {
            model,
            name: name.clone(),
        })
    }

    pub fn style_key_to_idx(&self, key: &StyleKey) -> Option<usize> {
        self.styles
            .keys()
            .enumerate()
            .find_map(|(i, k)| if k == key { Some(i) } else { None })
    }
}
