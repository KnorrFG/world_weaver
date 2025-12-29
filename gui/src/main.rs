use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::Parser;
use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use engine::{
    game::{Game, WorldDescription},
    image_model::{self, Flux2},
    llm::Claude,
    save_archive::SaveArchive,
};
use iced::{
    Task, debug,
    widget::{Column, button, column, text},
};
use log::debug;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use world_weaver::{
    CLAUDE_MODEL, Context, Gui, Message, PersistedState,
    cli::{self, Cli, NewGame},
    default_save_path, load_json_file, load_persisted_state, save_json_file, save_persisted_state,
    states,
};

pub fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    pretty_env_logger::init();

    let pstate = load_persisted_state()?.unwrap_or_default();

    iced::application(
        move || match create_or_load_game(pstate.clone(), &cli) {
            Ok((game, save)) => (Gui::new(game.clone(), save), Task::done(Message::Init)),
            Err(e) => panic!("{e:#?}"),
        },
        Gui::update,
        Gui::view,
    )
    .run()?;
    Ok(())
}

fn create_or_load_game(state: PersistedState, cli: &Cli) -> Result<(Game, SaveArchive)> {
    let claude_token = cli
        .claude_token
        .as_ref()
        .or(state.claude_token.as_ref())
        .ok_or_else(|| eyre!("No Claude-Token saved, please provide one via cli"))?
        .clone();
    let llm = Box::new(Claude::new(claude_token.clone(), CLAUDE_MODEL.into()));

    let model = state.current_img_model.unwrap_or(image_model::Model::Flux2);
    let key = state
        .img_model_tokens
        .get(&model.provider())
        .ok_or(eyre!("No token for {model}"))?;
    let imgmod = model.make(key.clone());

    match cli.command.as_ref() {
        Some(cli::Command::NewGame(cli::NewGame { world, player })) => {
            ensure!(world.exists(), "provided world doesn't exist");

            let world_desc = load_json_file(world)?;
            let game = Game::try_new(llm, imgmod, world_desc, player.clone())?;
            let save_path = default_save_path()?;
            fs::create_dir_all(save_path.parent().unwrap())?;
            let mut archive = SaveArchive::create(save_path)?;
            archive.write_game_data(&game.data)?;
            Ok((game, archive))
        }

        _ => {
            let save_path = default_save_path()?;
            ensure!(
                save_path.exists(),
                "No game running. Please start a new one via the NewGame command"
            );

            debug!("Loading save: {save_path:?}");
            let mut archive = SaveArchive::open(save_path)?;
            let game_data = archive.read_game_data()?;
            Ok((Game::load(llm, imgmod, game_data), archive))
        }
    }
}
