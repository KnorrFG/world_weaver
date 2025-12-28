use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use color_eyre::{Result, eyre::eyre};
use engine::{
    game::{Game, Image, TurnOutput},
    llm::OutputMessage,
    save_archive::SaveArchive,
};
use iced::{Element, Task, widget::text_editor};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub mod cli;
pub mod states;

const APP_NAME: &str = "World Weaver";

pub trait State: fmt::Debug {
    fn update(&mut self, event: Message, ctx: &mut Context) -> Result<StateCommand>;
    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, Message>;
    fn clone(&self) -> Box<dyn State>;
}

impl<T: std::ops::DerefMut<Target = dyn State> + fmt::Debug> State for T {
    fn update(&mut self, event: Message, ctx: &mut Context) -> Result<StateCommand> {
        self.deref_mut().update(event, ctx)
    }

    fn view<'a>(&'a self, ctx: &'a Context) -> Element<'a, Message> {
        self.deref().view(ctx)
    }

    fn clone(&self) -> Box<dyn State> {
        self.deref().clone()
    }
}

pub struct Gui {
    state: Box<dyn State>,
    ctx: Context,
}

impl Gui {
    pub fn new(game: Game, save: SaveArchive) -> Self {
        Gui {
            state: Box::new(states::Playing::new()),
            ctx: Context { game, save },
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let cmd = self.state.update(message, &mut self.ctx);

        match cmd {
            Ok(cmd) => {
                if let Some(new_state) = cmd.transition {
                    self.state = new_state;
                }
                cmd.task.unwrap_or(Task::none())
            }
            Err(e) => {
                self.state = Box::new(states::Error {
                    message: e.to_string(),
                    parent_state: Some(self.state.clone()),
                });
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        self.state.view(&self.ctx)
    }
}

pub struct Context {
    game: Game,
    save: SaveArchive,
}

#[derive(Debug, Clone)]
pub struct StringError(pub String);

impl fmt::Display for StringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<color_eyre::Report> for StringError {
    fn from(value: color_eyre::Report) -> Self {
        Self(value.to_string())
    }
}

impl std::error::Error for StringError {}

#[derive(Debug, Clone)]
pub enum Message {
    OutputComplete(Result<TurnOutput, StringError>),
    NewTextFragment(Result<String, StringError>),
    ImageReady(Result<Image, StringError>),
    Init,
    UpdateActionText(text_editor::Action),
    UpdateGMInstructionText(text_editor::Action),

    ProposedActionButtonPressed(String),
    Submit,
    SummaryFinished(Result<Option<OutputMessage>, StringError>),

    PrevTurnButtonPressed,
    NextTurnButtonPressed,
    UpdateTurnInput(String),
    GotoTurnPressed,
    GoToCurrentTurn,
    ErrorConfirmed,
}

#[derive(Debug, Default)]
pub struct StateCommand {
    pub task: Option<Task<Message>>,
    pub transition: Option<Box<dyn State>>,
}

pub mod cmd {
    use super::*;

    pub fn none() -> Result<StateCommand> {
        Ok(StateCommand::default())
    }

    pub fn task(t: Task<Message>) -> Result<StateCommand> {
        Ok(StateCommand {
            task: Some(t),
            transition: None,
        })
    }

    pub fn transition(s: impl State + 'static) -> Result<StateCommand> {
        Ok(StateCommand {
            task: None,
            transition: Some(Box::new(s)),
        })
    }

    pub fn transition_with_task(s: impl State + 'static, t: Task<Message>) -> Result<StateCommand> {
        Ok(StateCommand {
            task: Some(t),
            transition: Some(Box::new(s)),
        })
    }
}

pub const CLAUDE_MODEL: &str = "claude-sonnet-4-5";
pub const DEFAULT_SAVE_NAME: &str = "default_save";
pub const PERSISTENT_INFO_NAME: &str = "persisted_info";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PersistedState {
    pub claude_token: Option<String>,
    pub flux_token: Option<String>,
}

pub fn load_json_file<T: DeserializeOwned>(world: &Path) -> Result<T> {
    let src = fs::read_to_string(world)?;
    Ok(serde_json::from_str(&src)?)
}

pub fn save_json_file<T: Serialize>(path: &Path, x: &T) -> Result<()> {
    Ok(fs::write(path, &serde_json::to_string(x)?)?)
}

pub fn data_dir() -> Result<PathBuf> {
    Ok(dirs::data_dir()
        .ok_or(eyre!("Couldn't find data dir"))?
        .join(APP_NAME))
}
pub fn persistent_state_path() -> Result<PathBuf> {
    Ok(data_dir()?.join(PERSISTENT_INFO_NAME))
}

pub fn default_save_path() -> Result<PathBuf> {
    Ok(data_dir()?.join(DEFAULT_SAVE_NAME))
}

pub fn load_persisted_state() -> Result<Option<PersistedState>> {
    let path = persistent_state_path()?;
    if !path.exists() {
        Ok(None)
    } else {
        load_json_file(&path).map(Some)
    }
}

pub fn save_persisted_state(ps: &PersistedState) -> Result<()> {
    let path = persistent_state_path()?;
    save_json_file(&path, ps)?;
    Ok(())
}

macro_rules! elem_list {
    ($($elems:expr),+ $(,)?) => {
        [$(iced::Element::from($elems)),*]
    };
}
pub(crate) use elem_list;

use crate::states::Error;
