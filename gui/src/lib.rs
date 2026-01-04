use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use color_eyre::{
    Result,
    eyre::{WrapErr as _, eyre},
};
use iced::{
    Element, Font, Length, Task, Theme,
    font::{self},
    padding,
    widget::{container, scrollable, text},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    context::Config,
    message::Message,
    state::{Modal, State, StateExt, options_menu::OptionsMenu},
};

pub mod cli;
pub mod context;
pub mod message;
pub mod state;

const APP_NAME: &str = "World Weaver";

pub struct Gui {
    state: Box<dyn State>,
    ctx: context::Context,
}

impl Gui {
    pub fn new(mb_config: Option<Config>, opt_menu: OptionsMenu) -> Self {
        if let Some(cfg) = mb_config {
            Gui {
                state: Box::new(state::MainMenu::try_new().expect("Couldn't start Game")),
                ctx: context::Context::from_config(cfg),
            }
        } else {
            Gui {
                state: Modal::message(
                    opt_menu.boxed(),
                    "Welcome",
                    indoc::indoc! {"
                    Hi, since this is your first time starting World Weaver, please configure the
                    required API-keys. You only need keys for the Providers you actually use, so at
                    minimum, you will need two API-keys: one for Anthropic and one for the Image model provider
                    of your choice.
                    "
                    },
                ).boxed(),
                ctx: context::Context::from_config(Config::default()),
            }
        }
    }

    pub fn update(&mut self, message: message::Message) -> Task<message::Message> {
        match self.try_update(message) {
            Ok(task) => task,
            Err(e) => {
                self.state = Modal::message(self.state.clone(), "Error", format!("{e:?}")).boxed();
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

pub const CLAUDE_MODEL: &str = "claude-sonnet-4-5";
pub const PERSISTENT_INFO_NAME: &str = "persisted_info";

pub fn load_json_file<T: DeserializeOwned>(world: &Path) -> Result<T> {
    let src = fs::read_to_string(world)?;
    Ok(serde_json::from_str(&src)?)
}

pub fn save_json_file<T: Serialize>(path: &Path, x: &T) -> Result<()> {
    Ok(fs::write(path, &serde_json::to_string(x)?)?)
}

pub fn load_ron_file<T: DeserializeOwned>(world: &Path) -> Result<T> {
    let src = fs::read_to_string(world)?;
    Ok(ron::from_str(&src)?)
}

pub fn save_ron_file<T: Serialize>(path: &Path, x: &T) -> Result<()> {
    Ok(fs::write(path, &ron::to_string(x)?)?)
}

pub fn data_dir() -> Result<PathBuf> {
    Ok(dirs::data_dir()
        .ok_or(eyre!("Couldn't find data dir"))?
        .join(APP_NAME))
}

pub fn saves_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("saves"))
}

pub fn worlds_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("worlds"))
}

pub fn styles_dir() -> Result<PathBuf> {
    Ok(data_dir()?.join("styles"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(dirs::config_local_dir()
        .ok_or(eyre!("Couldn't get config dir"))?
        .join("world_weaver.ron"))
}

pub fn active_game_save_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("active_game"))
}

pub fn load_config() -> Result<Option<Config>> {
    let path = config_path()?;
    if !path.exists() {
        Ok(None)
    } else {
        load_ron_file(&path).map(Some)
    }
}

pub fn save_config(ps: &Config) -> Result<()> {
    let path = config_path()?;
    save_ron_file(&path, ps)?;
    Ok(())
}

macro_rules! elem_list {
    ($($elems:expr),+ $(,)?) => {
        [$(iced::Element::from($elems)),*]
    };
}
pub(crate) use elem_list;

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

fn bold_text<'a>(t: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    iced::widget::text(t).font(bold_default_font())
}

fn bold_default_font() -> Font {
    Font {
        weight: font::Weight::Bold,
        ..Font::DEFAULT
    }
}

fn top_level_container<'a, T: Send + 'static>(
    elem: impl Into<Element<'a, T>>,
) -> container::Container<'a, T> {
    container(
        container(scrollable(
            container(elem).padding(padding::all(10).right(20)),
        ))
        .padding(20)
        .max_width(800),
    )
    .center(Length::Fill)
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
