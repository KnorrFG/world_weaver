use color_eyre::{Result, eyre::ensure};
use engine::{game::Game, save_archive::SaveArchive};
use iced::{
    Length,
    alignment::Horizontal,
    debug,
    widget::{button, column, container},
};
use log::debug;

use crate::{
    State, TryIntoExt, active_game_save_path,
    context::{Context, game_context::GameContext},
    elem_list,
    message::{UiMessage, ui_messages::MainMenu as MyMessage},
    state::{self, Playing, StateCommand, cmd},
};

#[derive(Debug, Clone)]
pub struct MainMenu {
    active_game_exists: bool,
}

impl MainMenu {
    pub fn new() -> Result<Self> {
        Ok(MainMenu {
            active_game_exists: active_game_save_path()?.exists(),
        })
    }
}
impl State for MainMenu {
    fn update(
        &mut self,
        event: UiMessage,
        ctx: &mut Context,
    ) -> color_eyre::eyre::Result<StateCommand> {
        let msg: MyMessage = event.try_into_ex()?;
        use MyMessage::*;
        match msg {
            Continue => {
                ctx.game = None;
                let save_path = active_game_save_path()?;
                ensure!(
                    save_path.exists(),
                    "No game running. Please start a new one via the NewGame command"
                );

                debug!("Loading save: {save_path:?}");
                let mut archive = SaveArchive::open(save_path)?;
                let game_data = archive.read_game_data()?;
                let game = Game::load(
                    ctx.config.get_llm(),
                    ctx.config.get_image_model()?,
                    game_data,
                );
                ctx.game = Some(GameContext::try_new(game, archive)?);
                cmd::transition(Playing::new())
            }
            WorldsMenu => cmd::transition(state::WorldMenu::try_new()?),
            Options => todo!(),
            Save => todo!(),
            Load => todo!(),
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> iced::Element<'a, crate::message::UiMessage> {
        let mut buttons = vec![];
        if self.active_game_exists {
            buttons.extend(elem_list![
                button("Continue").on_press(MyMessage::Continue.into()),
                button("Save").on_press(MyMessage::Save.into()),
            ]);
        }

        buttons.extend(elem_list![
            button("New Game / Worlds").on_press(MyMessage::WorldsMenu.into()),
            button("Load Game").on_press(MyMessage::Load.into()),
            button("Options").on_press(MyMessage::Options.into()),
        ]);

        container(column(buttons).spacing(10).align_x(Horizontal::Center))
            .center(Length::Fill)
            .into()
    }

    fn clone(&self) -> Box<dyn State> {
        Box::new(Clone::clone(self))
    }
}
