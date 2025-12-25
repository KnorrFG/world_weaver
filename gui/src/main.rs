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
    llm::Claude,
};
use iced::{
    Task,
    widget::{Column, button, column, text},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use world_weaver::{
    CLAUDE_MODEL, Context, Gui, Message, PersistedState,
    cli::{self, Cli, NewGame},
    default_save_path, load_json_file, load_persisted_state, save_json_file, save_persisted_state,
    states,
};

pub fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let game = create_or_load_game(load_persisted_state()?, &cli)?;
    pretty_env_logger::init();

    iced::application(
        move || {
            (
                Gui::new(game.clone(), default_save_path().unwrap()),
                Task::done(Message::Init),
            )
        },
        Gui::update,
        Gui::view,
    )
    .run()?;
    Ok(())
}

fn create_or_load_game(mb_state: Option<PersistedState>, cli: &Cli) -> Result<Game> {
    let token = cli
        .claude_token
        .as_ref()
        .or(mb_state.as_ref().map(|s| &s.claude_token))
        .ok_or_else(|| eyre!("No Token saved, please provide one via cli"))?
        .clone();

    let llm = Box::new(Claude::new(token.clone(), CLAUDE_MODEL.into()));

    match cli.command.as_ref() {
        Some(cli::Command::NewGame(cli::NewGame { world, player })) => {
            ensure!(world.exists(), "provided world doesn't exist");

            let world_desc = load_json_file(world)?;
            let game = Game::try_new(llm, world_desc, player.clone())?;
            let save_path = default_save_path()?;
            save_json_file(&save_path, game.get_data())?;
            save_persisted_state(&PersistedState {
                claude_token: token,
                active_save: save_path,
            })?;
            Ok(game)
        }

        _ => {
            let state = mb_state.ok_or_else(|| {
                eyre!("No game running. Please start a new one via the NewGame command")
            })?;

            let game_data = load_json_file(&state.active_save)?;
            Ok(Game::load(llm, game_data))
        }
    }
}
