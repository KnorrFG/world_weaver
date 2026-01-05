use std::{fs, path::PathBuf};

use color_eyre::Result;
use engine::game::WorldDescription;
use iced::{
    Length,
    widget::{Space, button, column, row, space, text},
};
use log::debug;

use crate::{
    TryIntoExt, bold_text, elem_list, load_ron_file,
    message::ui_messages::WorldMenu as MyMessage,
    state::{MainMenu, Modal, State, WorldEditor, cmd, start_new_game::StartNewGame},
    top_level_container, worlds_dir,
};

const EXAMPLE_WORLD: &str = include_str!("../../../Neon_Shadows.ron");

#[derive(Clone, Debug)]
pub struct WorldMenu {
    worlds: Vec<(PathBuf, WorldDescription)>,
}

impl WorldMenu {
    pub fn try_new() -> Result<Self> {
        let dir = worlds_dir()?;

        if !dir.exists() {
            fs::create_dir_all(&dir)?;
            fs::write(dir.join("Neon_Shadows.ron"), EXAMPLE_WORLD)?;
        }

        debug!("World-files (in {dir:?}):",);
        let worlds = fs::read_dir(dir)?
            .map(|p| {
                let p = p?;
                debug!("{:?}", p.path());
                Ok((p.path(), load_ron_file::<WorldDescription>(&p.path())?))
            })
            .collect::<Result<Vec<_>>>()?;

        debug!(
            "Loaded worlds:\n{}",
            worlds
                .iter()
                .map(|w| w.1.name.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
        Ok(Self { worlds })
    }
}

impl super::State for WorldMenu {
    fn update(
        &mut self,
        event: crate::message::UiMessage,
        _ctx: &mut crate::context::Context,
    ) -> color_eyre::eyre::Result<super::StateCommand> {
        let msg: MyMessage = event.try_into_ex()?;
        use MyMessage::*;
        match msg {
            NewWorld => cmd::transition(WorldEditor::for_worlds_menu(None)),
            StartWorld(i) => cmd::transition(StartNewGame::new(self.worlds[i].1.clone())),
            EditWorld(i) => cmd::transition(WorldEditor::for_worlds_menu(Some(&self.worlds[i].1))),
            Back => cmd::transition(MainMenu::try_new()?),
            DeleteWorld(i) => cmd::transition(Modal::confirm(
                State::clone(self),
                "Do you really want to delete this world?",
                Some(MyMessage::ConfirmDeleteWorld(i).into()),
                None,
            )),
            ConfirmDeleteWorld(i) => {
                let world = &self.worlds.remove(i);
                fs::remove_file(&world.0)?;
                cmd::none()
            }
        }
    }

    fn view<'a>(
        &'a self,
        _ctx: &'a crate::context::Context,
    ) -> iced::Element<'a, crate::message::UiMessage> {
        let mut tlc = Vec::from(elem_list![
            bold_text("Worlds").width(Length::Fill).center(),
            Space::new().height(30),
            row![
                space::horizontal(),
                button("New World").on_press(MyMessage::NewWorld.into()),
                button("Back").on_press(MyMessage::Back.into()),
                space::horizontal()
            ]
            .spacing(10)
        ]);

        for (i, world) in self.worlds.iter().enumerate() {
            tlc.push(
                row![
                    text(&world.1.name),
                    space::horizontal(),
                    button("delete").on_press(MyMessage::DeleteWorld(i).into()),
                    button("edit").on_press(MyMessage::EditWorld(i).into()),
                    button("start").on_press(MyMessage::StartWorld(i).into())
                ]
                .spacing(10)
                .into(),
            );
        }

        top_level_container(
            column(tlc)
                .spacing(20)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .into()
    }

    fn clone(&self) -> Box<dyn super::State> {
        Box::new(Clone::clone(self))
    }
}
