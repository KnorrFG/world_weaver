use std::fs;

use clap::Parser;
use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use engine::{
    game::Game,
    image_model::{self},
    llm::Claude,
    save_archive::SaveArchive,
};
use log::debug;

use world_weaver::{
    CLAUDE_MODEL, Config, Gui, active_game_save_path,
    cli::{self, Cli},
    load_json_file,
};

pub fn main() -> Result<()> {
    pretty_env_logger::init();

    iced::application(Gui::new, Gui::update, Gui::view)
        // .theme(Gui::theme)
        .run()?;
    Ok(())
}
