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

    let pstate = load_persisted_state()?;

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

fn create_or_load_game(mb_state: Option<PersistedState>, cli: &Cli) -> Result<(Game, SaveArchive)> {
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
            fs::create_dir_all(save_path.parent().unwrap())?;
            let mut archive = SaveArchive::create(save_path)?;
            archive.write_game_data(game.get_data())?;
            save_persisted_state(&PersistedState {
                claude_token: token,
            })?;
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
            Ok((Game::load(llm, game_data), archive))
        }
    }
}
