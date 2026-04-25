use color_eyre::eyre::Result;
use engine::{
    game::{Game, WorldDescription},
    save_archive::SaveArchive,
};
use iced::{
    Font, Length, Task,
    widget::{Space, button, column, text},
};

use crate::{
    Config, TryIntoExt, bold_default_font, load_remembered_saves,
    save_active_game_save_path, save_remembered_saves,
    context::{Context, game_context::GameContext},
    elem_list,
    message::{ContextMessage, Message, UiMessage, ui_messages::StartNewGame as MyMessage},
    state::{Playing, cmd},
    top_level_container,
};

use super::{State, StateCommand};

#[derive(Debug, Clone)]
pub struct StartNewGame {
    world: WorldDescription,
}

impl StartNewGame {
    pub fn new(world: WorldDescription) -> Self {
        Self { world }
    }

    fn create_game(&self, c: String, config: &Config) -> Result<Game> {
        Game::try_new(
            config.get_llm()?,
            config.get_image_model()?,
            self.world.clone(),
            c,
            config.active_style().cloned(),
        )
    }

    fn default_save_filename(&self, character: &str) -> String {
        let basename = format!("{}_{}", self.world.name, character)
            .replace(' ', "_")
            .to_lowercase();
        format!("{basename}.wwsave")
    }
}

impl State for StartNewGame {
    fn update(&mut self, event: UiMessage, ctx: &mut Context) -> Result<StateCommand> {
        use MyMessage::*;
        match event.try_into_ex()? {
            Selected(c) => {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("World Weaver saves", &["wwsave"])
                    .set_file_name(self.default_save_filename(&c))
                    .save_file()
                else {
                    return cmd::none();
                };

                ctx.game = None;
                let game = self.create_game(c, &ctx.config)?;
                let archive = SaveArchive::create(&path)?;
                ctx.game = Some(GameContext::try_new(game, archive)?);

                let mut remembered_saves = load_remembered_saves()?;
                if !remembered_saves.contains(&path) {
                    remembered_saves.push(path.clone());
                    save_remembered_saves(&remembered_saves)?;
                }
                save_active_game_save_path(&path)?;

                cmd::transition_with_task::<Message>(
                    Playing::new(),
                    Task::done(ContextMessage::Init.into()),
                )
            }
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> iced::Element<'a, UiMessage> {
        let mut tlc = Vec::from(elem_list![
            text!("New Game - {}", self.world.name)
                .font(bold_default_font())
                .size(20),
            text("Select a Character:"),
            Space::new().height(20)
        ]);

        for (name, description) in &self.world.pc_descriptions {
            tlc.extend(elem_list![
                text(name)
                    .font(Font {
                        weight: iced::font::Weight::Semibold,
                        ..Font::DEFAULT
                    })
                    .size(16),
                text(&description.description),
                button("Select").on_press(MyMessage::Selected(name.clone()).into())
            ]);
        }

        top_level_container(
            column(tlc)
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(20),
        )
        .into()
    }

    fn clone(&self) -> Box<dyn State> {
        Box::new(Clone::clone(self))
    }
}
