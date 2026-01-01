use std::{collections::HashMap, fs, path::PathBuf};

use crate::{
    ElemHelper, TryIntoExt, bold_text,
    context::Context,
    elem_list,
    message::{self, UiMessage, ui_messages::WorldEditor as MyMessage},
    save_json_file,
    state::{Modal, StateExt, WorldMenu, cmd, start_new_game::StartNewGame},
    top_level_container, worlds_dir,
};

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use engine::game::WorldDescription;
use iced::{
    Font, Length, Task, padding,
    widget::{
        Space, button, column, container, row, rule, scrollable, space, text, text_editor,
        text_input,
    },
};

use super::State;

#[derive(Debug, Clone, Default)]
pub struct WorldEditor {
    name: String,
    description: text_editor::Content,
    init_action: text_editor::Content,
    characters: HashMap<String, text_editor::Content>,
}

impl WorldEditor {
    pub fn new() -> Self {
        Self::default()
    }

    fn try_save_world(&self) -> Result<WorldDescription> {
        let path = self.current_save_path()?;
        ensure!(!path.exists(), "A world with that name alread exists");
        let world = WorldDescription {
            name: self.name.clone(),
            main_description: self.description.text(),
            pc_descriptions: self
                .characters
                .iter()
                .map(|(k, v)| (k.clone(), v.text()))
                .collect(),
            init_action: self.init_action.text(),
        };
        fs::create_dir_all(path.parent().unwrap())?;
        save_json_file(&path, &world)?;
        Ok(world)
    }

    fn current_save_path(&self) -> Result<PathBuf> {
        Ok(worlds_dir()?.join(self.name.replace(" ", "_") + ".json"))
    }
}

impl State for WorldEditor {
    fn update(
        &mut self,
        event: UiMessage,
        ctx: &mut Context,
    ) -> color_eyre::eyre::Result<super::StateCommand> {
        use MyMessage::*;
        match event.try_into_ex()? {
            AddCharacterButton => cmd::transition(Modal::input(
                State::clone(self),
                "New Chacacter",
                "Character Name",
                |x| Task::done(MyMessage::AddCharacter(x).into()),
            )),
            AddCharacter(name) => {
                self.characters
                    .insert(name, text_editor::Content::default());
                cmd::none()
            }
            UpdateCharacter(name, a) => {
                self.characters
                    .get_mut(&name)
                    .ok_or(eyre!("Character name invalid"))?
                    .perform(a);
                cmd::none()
            }
            DescriptionUpdate(a) => {
                self.description.perform(a);
                cmd::none()
            }
            NameUpdate(n) => {
                self.name = n;
                cmd::none()
            }
            InitActionUpdate(a) => {
                self.init_action.perform(a);
                cmd::none()
            }
            Save => {
                self.try_save_world()?;
                cmd::transition(Modal::message(
                    State::clone(self),
                    "Info",
                    "Saving succesful",
                ))
            }
            SaveAndPlay => {
                let world = self.try_save_world()?;
                cmd::transition(StartNewGame::new(world))
            }
            Abort => cmd::transition(WorldMenu::try_new()?),
        }
    }

    fn view<'a>(&'a self, ctx: &'a Context) -> iced::Element<'a, UiMessage> {
        let mut tlc = Vec::from(elem_list![
            bold_text("New World").size(24).width(Length::Fill).center(),
            text_input("World name", &self.name).on_input(|n| MyMessage::NameUpdate(n).into()),
            text("Description:"),
            text_editor(&self.description).on_action(|a| MyMessage::DescriptionUpdate(a).into()),
            text("Initial Action:"),
            text_editor(&self.init_action).on_action(|a| MyMessage::InitActionUpdate(a).into()),
            Space::new().height(20),
            rule::horizontal(2),
            bold_text("Characters")
                .size(20)
                .width(Length::Fill)
                .center(),
        ]);

        let char_col = self
            .characters
            .iter()
            .map(|(name, content)| {
                column![
                    text(name)
                        .font(Font {
                            weight: iced::font::Weight::Semibold,
                            ..Font::DEFAULT
                        })
                        .size(16),
                    text_editor(content)
                        .on_action(|a| MyMessage::UpdateCharacter(name.clone(), a).into()),
                ]
                .spacing(10)
                .into()
            })
            .chain([button("Add Character")
                .on_press(MyMessage::AddCharacterButton.into())
                .into()]);

        tlc.push(
            container(column(char_col).spacing(20))
                .padding([30, 0])
                .into(),
        );

        tlc.push(
            row![
                space::horizontal(),
                button("Abort").on_press(MyMessage::Abort.into()),
                button("Save").on_press(MyMessage::Save.into()),
                button("Save and play").on_press(MyMessage::SaveAndPlay.into()),
                space::horizontal(),
            ]
            .spacing(10)
            .width(Length::Fill)
            .into(),
        );

        top_level_container(
            column(tlc)
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(20),
        )
        .into()
    }

    fn clone(&self) -> Box<dyn State> {
        Clone::clone(self).boxed()
    }
}
