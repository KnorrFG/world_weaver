use std::{fs, path::{Path, PathBuf}};

use clap::{Parser, Subcommand};
use color_eyre::{Result, eyre::eyre};
use engine::{
    game::{TurnInput, WorldDescription},
    save_archive::SaveArchive,
    world_markdown::{world_from_markdown, world_to_markdown},
};
use serde::Deserialize;

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    PrintActiveGameRequest,
    ExportWorldsMarkdown {
        target_dir: PathBuf,
    },
}

pub fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    match cli
        .command
        .ok_or(eyre!("No command given. Try `print-active-game-request` or `export-worlds-markdown`"))?
    {
        Command::PrintActiveGameRequest => print_active_game_request(),
        Command::ExportWorldsMarkdown { target_dir } => export_worlds_markdown(&target_dir),
    }
}

fn print_active_game_request() -> Result<()> {
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

fn export_worlds_markdown(target_dir: &Path) -> Result<()> {
    fs::create_dir_all(target_dir)?;

    for remembered in load_remembered_worlds()? {
        let path = remembered.path;
        let src = fs::read_to_string(&path)?;
        let world: WorldDescription = world_from_markdown(&src)?;
        let output_name = path
            .file_stem()
            .ok_or(eyre!("World file without file stem: {path:?}"))?;
        let output_path = target_dir.join(output_name).with_extension("md");
        fs::write(output_path, world_to_markdown(&world))?;
    }

    Ok(())
}

pub fn data_dir() -> Result<PathBuf> {
    Ok(dirs::data_dir()
        .ok_or(eyre!("Couldn't find data dir"))?
        .join("World Weaver"))
}

pub fn remembered_worlds_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("remembered_worlds.ron"))
}

pub fn active_game_save_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("active_game"))
}

fn load_remembered_worlds() -> Result<Vec<RememberedWorld>> {
    let path = remembered_worlds_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let src = fs::read_to_string(path)?;
    Ok(ron::from_str(&src)?)
}

#[derive(Debug, Deserialize)]
struct RememberedWorld {
    path: PathBuf,
}
