use std::{
    collections::HashMap,
    fmt, fs,
    path::{Path, PathBuf},
};

use color_eyre::{
    Result,
    eyre::{WrapErr as _, eyre},
};
use engine::{game::Game, image_model, save_archive::SaveArchive};
use iced::{
    Element, Font, Task, Theme,
    font::{self},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub mod cli;
pub mod message;
pub mod state;

const APP_NAME: &str = "World Weaver";

pub struct Gui {
    state: Box<dyn State>,
    ctx: Context,
}

impl Gui {
    pub fn new(game: Game, save: SaveArchive) -> Self {
        Gui {
            state: Box::new(state::Playing::new()),
            ctx: Context { game, save },
        }
    }

    pub fn update(&mut self, message: message::Message) -> Task<message::Message> {
        match self.try_update(message) {
            Ok(task) => task,
            Err(e) => {
                self.state = Modal::message(self.state.clone(), "Error", e.to_string()).boxed();
                Task::none()
            }
        }
    }

    fn try_update(&mut self, message: Message) -> Result<Task<Message>> {
        match message {
            Message::Ui(ui_message) => {
                let cmd = self.state.update(ui_message, &mut self.ctx)?;
                if let Some(new_state) = cmd.transition {
                    self.state = new_state;
                }
                Ok(cmd
                    .task
                    .map(|t| t.map(Message::from))
                    .unwrap_or(Task::none()))
            }
            Message::Context(context_message) => self.ctx.update(context_message),
        }
    }

    pub fn view(&self) -> Element<'_, message::Message> {
        self.state.view(&self.ctx).map(|m| m.into())
    }

    pub fn theme(&self) -> Theme {
        Theme::SolarizedLight
    }
}

pub struct Context {
    game: Game,
    save: SaveArchive,
}

impl Context {
    fn update(&self, message: ContextMessage) -> Result<Task<Message>> {
        todo!()
    }
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

pub const CLAUDE_MODEL: &str = "claude-sonnet-4-5";
pub const DEFAULT_SAVE_NAME: &str = "default_save";
pub const PERSISTENT_INFO_NAME: &str = "persisted_info";

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PersistedState {
    pub claude_token: Option<String>,
    pub current_img_model: Option<image_model::Model>,
    pub img_model_tokens: HashMap<image_model::ModelProvider, String>,
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

use crate::{
    message::{ContextMessage, Message},
    state::{Modal, State, StateExt},
};

pub trait ElemHelper<'a, T> {
    fn into_elem(self) -> Element<'a, T>;
}

impl<'a, ElemT, T: Into<Element<'a, ElemT>>> ElemHelper<'a, ElemT> for T {
    fn into_elem(self) -> Element<'a, ElemT> {
        self.into()
    }
}

fn italic_text(t: &str) -> iced::widget::Text<'_> {
    iced::widget::text(t).font(italic_default_font()).into()
}

fn italic_default_font() -> Font {
    Font {
        style: font::Style::Italic,
        ..Font::DEFAULT
    }
}

fn bold_text(t: &str) -> iced::widget::Text<'_> {
    iced::widget::text(t).font(bold_default_font())
}

fn bold_default_font() -> Font {
    Font {
        weight: font::Weight::Bold,
        ..Font::DEFAULT
    }
}

pub trait TryIntoExt<T> {
    fn try_into_ex(self) -> color_eyre::Result<T>;
}

impl<T, Target, E> TryIntoExt<Target> for T
where
    T: TryInto<Target, Error = E>,
    T: fmt::Debug,
    T: Clone,
    E: std::error::Error + Send + Sync + 'static,
{
    fn try_into_ex(self) -> color_eyre::Result<Target> {
        self.clone()
            .try_into()
            .with_context(|| format!("{self:#?}"))
    }
}
