use color_eyre::Result;
use engine::save_archive::SaveArchive;
use iced::{
    Length,
    alignment::Horizontal,
    widget::{button, column, container},
};

use crate::{
    State, TryIntoExt, load_active_game_save_path,
    context::Context,
    elem_list,
    message::{UiMessage, ui_messages::MainMenu as MyMessage},
    state::{
        self, Playing, StateCommand, WorldEditor, cmd, load_menu::LoadMenu, options_menu::OptionsMenu,
    },
};

#[derive(Debug, Clone)]
pub struct MainMenu {
    active_game_exists: bool,
}

impl MainMenu {
    pub fn try_new() -> Result<Self> {
        Ok(MainMenu {
            active_game_exists: load_active_game_save_path()?
                .map(|path| {
                    SaveArchive::open(&path)
                        .and_then(|mut archive| archive.read_game_data().map(|_| ()))
                        .is_ok()
                })
                .unwrap_or(false),
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
                if ctx.game.is_none() {
                    ctx.load_game()?;
                }
                cmd::transition(Playing::new())
            }
            RestartCurrentWorld => {
                let world = if let Some(gctx) = &ctx.game {
                    gctx.game.data.world_description.clone()
                } else {
                    ctx.load_game()?.data.world_description.clone()
                };
                cmd::transition(state::start_new_game::StartNewGame::new(world))
            }
            WorldsMenu => cmd::transition(state::WorldMenu::try_new()?),
            Load => cmd::transition(LoadMenu::try_new()?),
            Options => cmd::transition(OptionsMenu::new(&ctx.config)?),
            EditActiveWorld => {
                let world = if let Some(gctx) = &ctx.game {
                    &gctx.game.data.world_description
                } else {
                    &ctx.load_game()?.data.world_description
                };

                cmd::transition(WorldEditor::edit_running_world(world))
            }
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
                button("Restart current world")
                    .on_press(MyMessage::RestartCurrentWorld.into())
                    .width(button_w),
                button("Edit active world")
                    .on_press(MyMessage::EditActiveWorld.into())
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
