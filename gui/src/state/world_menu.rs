use std::fs;

use color_eyre::Result;
use engine::game::WorldDescription;
use iced::{
    Length,
    widget::{Space, button, column, row, space, text},
};
use log::debug;

use crate::{
    TryIntoExt, bold_text, elem_list, load_json_file,
    message::ui_messages::WorldMenu as MyMessage,
    state::{WorldEditor, cmd, start_new_game::StartNewGame},
    top_level_container, worlds_dir,
};

#[derive(Clone, Debug)]
pub struct WorldMenu {
    worlds: Vec<WorldDescription>,
}

impl WorldMenu {
    pub fn try_new() -> Result<Self> {
        let dir = worlds_dir()?;
        debug!("World-files (in {dir:?}):",);
        let worlds = fs::read_dir(dir)?
            .map(|p| {
                let p = p?;
                debug!("{:?}", p.path());
                load_json_file::<WorldDescription>(&p.path())
            })
            .collect::<Result<Vec<_>>>()?;

        debug!(
            "Loaded worlds:\n{}",
            worlds
                .iter()
                .map(|w| w.name.as_str())
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
        ctx: &mut crate::context::Context,
    ) -> color_eyre::eyre::Result<super::StateCommand> {
        let msg: MyMessage = event.try_into_ex()?;
        use MyMessage::*;
        match msg {
            NewWorld => cmd::transition(WorldEditor::new()),
            StartWorld(i) => cmd::transition(StartNewGame::new(self.worlds[i].clone())),
        }
    }

    fn view<'a>(
        &'a self,
        ctx: &'a crate::context::Context,
    ) -> iced::Element<'a, crate::message::UiMessage> {
        let mut tlc = Vec::from(elem_list![
            bold_text("Worlds").width(Length::Fill).center(),
            Space::new().height(30),
            row![
                space::horizontal(),
                button("New World").on_press(MyMessage::NewWorld.into()),
                space::horizontal()
            ]
        ]);

        for (i, world) in self.worlds.iter().enumerate() {
            tlc.push(
                row![
                    text(&world.name),
                    space::horizontal(),
                    button("start").on_press(MyMessage::StartWorld(i).into())
                ]
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
