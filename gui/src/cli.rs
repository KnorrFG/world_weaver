use std::path::PathBuf;

#[derive(Debug, clap::Parser)]
pub struct Cli {
    #[arg(short, long)]
    pub claude_token: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Doc comment
#[derive(Debug, clap::Subcommand)]
pub enum Command {
    /// Doc comment
    NewGame(NewGame),
}

#[derive(Debug, clap::Args)]
pub struct NewGame {
    pub world: PathBuf,
    pub player: String,
}
