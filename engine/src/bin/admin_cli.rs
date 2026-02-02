use std::path::PathBuf;

use color_eyre::{Result, eyre::eyre};
use engine::{game::TurnInput, save_archive::SaveArchive};

pub fn main() -> Result<()> {
    color_eyre::install()?;
    let mut archive = SaveArchive::open(active_game_save_path()?)?;
    let data = archive.read_game_data()?;
    let request = data.construct_request(&TurnInput::default(), "");

    println!("# System Message\n{}", request.system.unwrap());
    println!("# Messages");
    for m in request.messages {
        println!("{}", m.content);
    }

    Ok(())
}

pub fn data_dir() -> Result<PathBuf> {
    Ok(dirs::data_dir()
        .ok_or(eyre!("Couldn't find data dir"))?
        .join("World Weaver"))
}

pub fn active_game_save_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("active_game"))
}
