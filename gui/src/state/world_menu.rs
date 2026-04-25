use std::path::PathBuf;

use color_eyre::Result;
use engine::{game::WorldDescription, world_markdown::world_from_markdown};
use iced::{
    Length,
    widget::{Space, button, column, row, space, text, tooltip},
};
use log::debug;

use crate::{
    RememberedWorld, TryIntoExt, bold_text, elem_list, load_remembered_worlds, save_remembered_worlds,
    message::ui_messages::WorldMenu as MyMessage,
    state::{MainMenu, WorldEditor, cmd, start_new_game::StartNewGame},
    top_level_container,
};

#[derive(Clone, Debug)]
pub struct WorldMenu {
    worlds: Vec<RememberedWorldEntry>,
}

#[derive(Clone, Debug)]
struct RememberedWorldEntry {
    path: PathBuf,
    last_known_name: String,
    loaded_world: Option<WorldDescription>,
}

impl RememberedWorldEntry {
    fn load(world: RememberedWorld) -> Self {
        let loaded_world = std::fs::read_to_string(&world.path)
            .ok()
            .and_then(|src| world_from_markdown(&src).ok());
        let last_known_name = loaded_world
            .as_ref()
            .map(|world| world.name.clone())
            .unwrap_or(world.last_known_name);

        Self {
            path: world.path,
            last_known_name,
            loaded_world,
        }
    }

    fn display_name(&self) -> &str {
        self.loaded_world
            .as_ref()
            .map(|world| world.name.as_str())
            .unwrap_or(&self.last_known_name)
    }

    fn remember(&self) -> RememberedWorld {
        RememberedWorld {
            path: self.path.clone(),
            last_known_name: self.display_name().to_string(),
        }
    }
}

impl WorldMenu {
    pub fn try_new() -> Result<Self> {
        let worlds = load_remembered_worlds()?
            .into_iter()
            .map(RememberedWorldEntry::load)
            .collect::<Vec<_>>();

        debug!(
            "Remembered worlds:\n{}",
            worlds
                .iter()
                .map(|world| format!("{} -> {:?}", world.display_name(), world.path))
                .collect::<Vec<_>>()
                .join("\n")
        );

        Ok(Self { worlds })
    }

    fn write_remembered_worlds_index(&self) -> Result<()> {
        let remembered = self
            .worlds
            .iter()
            .map(RememberedWorldEntry::remember)
            .collect::<Vec<_>>();
        save_remembered_worlds(&remembered)
    }

    fn open_world_via_dialog(&mut self) -> Result<()> {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("World Weaver worlds", &["ww.md"])
            .add_filter("Markdown", &["md"])
            .pick_file()
        else {
            return Ok(());
        };

        let src = std::fs::read_to_string(&path)?;
        let world = world_from_markdown(&src)?;

        if let Some(existing) = self.worlds.iter_mut().find(|entry| entry.path == path) {
            existing.last_known_name = world.name.clone();
            existing.loaded_world = Some(world);
        } else {
            self.worlds.push(RememberedWorldEntry {
                path,
                last_known_name: world.name.clone(),
                loaded_world: Some(world),
            });
        }

        self.write_remembered_worlds_index()
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
            OpenWorld => {
                self.open_world_via_dialog()?;
                cmd::none()
            }
            StartWorld(i) => {
                let world = self.worlds[i]
                    .loaded_world
                    .clone()
                    .expect("disabled start button should prevent missing world start");
                cmd::transition(StartNewGame::new(world))
            }
            EditWorld(i) => {
                let world = self.worlds[i]
                    .loaded_world
                    .as_ref()
                    .expect("disabled edit button should prevent missing world edit");
                cmd::transition(WorldEditor::for_worlds_menu(Some((
                    self.worlds[i].path.clone(),
                    world,
                ))))
            }
            Back => cmd::transition(MainMenu::try_new()?),
            ForgetWorld(i) => {
                self.worlds.remove(i);
                self.write_remembered_worlds_index()?;
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
                button("Open...").on_press(MyMessage::OpenWorld.into()),
                button("New World").on_press(MyMessage::NewWorld.into()),
                button("Back").on_press(MyMessage::Back.into()),
                space::horizontal()
            ]
            .spacing(10)
        ]);

        for (i, world) in self.worlds.iter().enumerate() {
            let is_available = world.loaded_world.is_some();
            let warning: iced::Element<'_, crate::message::UiMessage> = if is_available {
                Space::new()
                    .width(Length::Shrink)
                    .height(Length::Shrink)
                    .into()
            } else {
                tooltip(
                    text("⚠"),
                    "This world file is missing or unreadable.",
                    tooltip::Position::Top,
                )
                .into()
            };
            let edit_button = if is_available {
                button("edit").on_press(MyMessage::EditWorld(i).into())
            } else {
                button("edit")
            };
            let start_button = if is_available {
                button("start").on_press(MyMessage::StartWorld(i).into())
            } else {
                button("start")
            };

            tlc.push(
                row![
                    warning,
                    column![
                        text(world.display_name()),
                        text(world.path.display().to_string()).size(14)
                    ]
                    .spacing(4),
                    space::horizontal(),
                    button("forget").on_press(MyMessage::ForgetWorld(i).into()),
                    edit_button,
                    start_button
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
