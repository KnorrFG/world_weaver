use std::fs;

use color_eyre::{
    Result,
    eyre::ensure,
};
use engine::{game::Game, save_archive::SaveArchive};
use iced::{
    Length, Task,
    alignment::Horizontal,
    widget::{button, column, container},
};
use log::debug;

use crate::{
    State, TryIntoExt, active_game_save_path,
    context::{Context, game_context::GameContext},
    elem_list,
    message::{UiMessage, ui_messages::MainMenu as MyMessage},
    saves_dir,
    state::{self, Modal, Playing, StateCommand, cmd, load_menu::LoadMenu},
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
            SaveButton => cmd::transition(Modal::input(
                State::clone(self),
                "Save name",
                "Save name",
                |s| Task::done(Save(s).into()),
            )),
            Save(save_name) => {
                let save_dir = saves_dir()?;
                fs::create_dir_all(&save_dir)?;
                let save_path = save_dir.join(&save_name);
                if let Some(gctx) = &mut ctx.game {
                    gctx.save.write_to(&save_path)?;
                } else {
                    fs::copy(active_game_save_path()?, save_path)?;
                }
                cmd::transition(Modal::message(
                    State::clone(self),
                    "Info",
                    "Saving Successful",
                ))
            }
            Load => cmd::transition(LoadMenu::try_new()?),
            Options => todo!(),
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> iced::Element<'a, crate::message::UiMessage> {
        let button_w = 200;
        let mut buttons = vec![];
        if self.active_game_exists {
            buttons.extend(elem_list![
                button("Continue")
                    .on_press(MyMessage::Continue.into())
                    .width(button_w),
                button("Save")
                    .on_press(MyMessage::SaveButton.into())
                    .width(button_w),
            ]);
        }

        buttons.extend(elem_list![
            button("New Game / Worlds")
                .on_press(MyMessage::WorldsMenu.into())
                .width(button_w),
            button("Load Game")
                .on_press(MyMessage::Load.into())
                .width(button_w),
            button("Options")
                .on_press(MyMessage::Options.into())
                .width(button_w),
        ]);

        container(column(buttons).spacing(10).align_x(Horizontal::Center))
            .center(Length::Fill)
            .into()
    }

    fn clone(&self) -> Box<dyn State> {
        Box::new(Clone::clone(self))
    }
}
