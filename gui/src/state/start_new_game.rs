use color_eyre::eyre::{Result, eyre};
use engine::{
    game::{Game, WorldDescription},
    image_model,
    llm::Claude,
    save_archive::SaveArchive,
};
use iced::{
    Font, Length, Task,
    widget::{Space, button, column, text},
};

use crate::{
    CLAUDE_MODEL, Config, TryIntoExt, active_game_save_path, bold_default_font, bold_text,
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
        Ok(Game::try_new(
            config.get_llm(),
            config.get_image_model()?,
            self.world.clone(),
            c,
        )?)
    }
}

impl State for StartNewGame {
    fn update(&mut self, event: UiMessage, ctx: &mut Context) -> Result<StateCommand> {
        use MyMessage::*;
        match event.try_into_ex()? {
            Selected(c) => {
                ctx.game = None;
                let game = self.create_game(c, &ctx.config)?;
                let archive = SaveArchive::create(active_game_save_path()?)?;
                ctx.game = Some(GameContext::try_new(game, archive)?);
                cmd::transition_with_task::<Message>(
                    Playing::new(),
                    Task::done(ContextMessage::Init.into()),
                )
            }
        }
    }

    fn view<'a>(&'a self, ctx: &'a Context) -> iced::Element<'a, UiMessage> {
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
                text(description),
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
