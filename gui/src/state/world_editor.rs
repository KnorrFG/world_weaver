use std::{collections::BTreeMap, fmt, fs, path::PathBuf, sync::Arc};

use crate::{
    TryIntoExt, bold_text,
    context::Context,
    elem_list,
    message::{UiMessage, ui_messages::WorldEditor as MyMessage},
    save_ron_file,
    state::{
        MainMenu, Modal, Playing, StateCommand, StateExt, WorldMenu, cmd,
        start_new_game::StartNewGame,
    },
    worlds_dir,
};

use color_eyre::{
    Result,
    eyre::{bail, ensure, eyre},
};
use engine::game::{PcDescription, WorldDescription};
use engine::world_markdown::world_to_markdown;
use iced::{
    Color, Font, Length, Task, padding,
    widget::{
        Space, button, column, container, row, rule, scrollable, space, text, text_editor,
        text_input,
    },
};

use super::State;

type ActionFnArc = Arc<dyn Fn(&mut WorldEditor, &mut Context) -> Result<StateCommand>>;

#[derive(Clone)]
pub struct WorldEditor {
    name: String,
    description: text_editor::Content,
    init_action: text_editor::Content,
    characters: BTreeMap<String, CharacterInputs>,
    editing_character_name: Option<(String, String)>,
    buttons: BTreeMap<String, ActionFnArc>,
}

#[derive(Debug, Clone, Default)]
struct CharacterInputs {
    description: text_editor::Content,
    initial_action: text_editor::Content,
}

impl fmt::Debug for WorldEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldEditor")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("init_action", &self.init_action)
            .field("characters", &self.characters)
            .field("editing_character_name", &self.editing_character_name)
            .field(
                "buttons",
                &self
                    .buttons
                    .keys()
                    .map(|k| (k, "<Closure>"))
                    .collect::<BTreeMap<_, _>>(),
            )
            .finish()
    }
}

impl WorldEditor {
    pub fn edit_running_world(wd: &WorldDescription) -> Self {
        Self {
            name: wd.name.clone(),
            description: text_editor::Content::with_text(&wd.main_description),
            init_action: text_editor::Content::with_text(&wd.init_action),
            characters: wd
                .pc_descriptions
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        CharacterInputs {
                            description: text_editor::Content::with_text(&v.description),
                            initial_action: text_editor::Content::with_text(&v.initial_action),
                        },
                    )
                })
                .collect(),
            editing_character_name: None,
            buttons: [
                (
                    "Abort".to_string(),
                    an(|_, _| cmd::transition(MainMenu::try_new()?)),
                ),
                (
                    "Save".to_string(),
                    an(|this, ctx| {
                        this.try_save_world_to_context(ctx)?;
                        cmd::transition(Modal::message(
                            State::clone(this),
                            "Info",
                            "Saving succesful",
                        ))
                    }),
                ),
                (
                    "Save and Play".to_string(),
                    an(|this, ctx| {
                        this.try_save_world_to_context(ctx)?;
                        cmd::transition(Playing::new())
                    }),
                ),
                (
                    "Export to File".to_string(),
                    an(|this, _| {
                        this.try_save_world(true)?;
                        cmd::transition(Modal::message(
                            State::clone(this),
                            "Info",
                            "Saving succesful",
                        ))
                    }),
                ),
                (
                    "Export Markdown".to_string(),
                    an(|this, _| {
                        this.try_export_markdown()?;
                        cmd::none()
                    }),
                ),
            ]
            .into(),
        }
    }

    pub fn for_worlds_menu(world: Option<&WorldDescription>) -> Self {
        // if wold_is some, we're editing an exisiting world,
        // and overwriting is OK, if it's none, we edit a new
        // world, and overwriting is not ok
        let exists_ok = world.is_some();
        let buttons = [
            (
                "Abort".to_string(),
                an(|_, _| cmd::transition(WorldMenu::try_new()?)),
            ),
            (
                "Save".to_string(),
                an(move |this, _| {
                    this.try_save_world(exists_ok)?;
                    cmd::transition(Modal::message(
                        State::clone(this),
                        "Info",
                        "Saving succesful",
                    ))
                }),
            ),
            (
                "Save and Play".to_string(),
                an(move |this, _| {
                    let world = this.try_save_world(exists_ok)?;
                    cmd::transition(StartNewGame::new(world))
                }),
            ),
            (
                "Export Markdown".to_string(),
                an(|this, _| {
                    this.try_export_markdown()?;
                    cmd::none()
                }),
            ),
        ]
        .into();

        if let Some(wd) = world {
            Self {
                name: wd.name.clone(),
                description: text_editor::Content::with_text(&wd.main_description),
                init_action: text_editor::Content::with_text(&wd.init_action),
                characters: wd
                    .pc_descriptions
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            CharacterInputs {
                                description: text_editor::Content::with_text(&v.description),
                                initial_action: text_editor::Content::with_text(&v.initial_action),
                            },
                        )
                    })
                    .collect(),
                editing_character_name: None,
                buttons,
            }
        } else {
            Self {
                name: "".into(),
                description: text_editor::Content::default(),
                init_action: text_editor::Content::default(),
                characters: BTreeMap::new(),
                editing_character_name: None,
                buttons,
            }
        }
    }

    fn try_save_world(&self, exists_ok: bool) -> Result<WorldDescription> {
        let path = self.current_save_path()?;
        ensure!(
            exists_ok || !path.exists(),
            "A world with that name alread exists"
        );
        let world = self.mk_world();
        fs::create_dir_all(path.parent().unwrap())?;
        save_ron_file(&path, &world)?;
        Ok(world)
    }

    fn mk_world(&self) -> WorldDescription {
        WorldDescription {
            name: self.name.clone(),
            main_description: self.description.text(),
            pc_descriptions: self
                .characters
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        PcDescription {
                            description: v.description.text(),
                            initial_action: v.initial_action.text(),
                        },
                    )
                })
                .collect(),
            init_action: self.init_action.text(),
        }
    }

    fn current_save_path(&self) -> Result<PathBuf> {
        Ok(worlds_dir()?.join(self.name.replace(" ", "_") + ".ron"))
    }

    fn try_save_world_to_context(&mut self, ctx: &mut Context) -> Result<()> {
        let Some(gctx) = &mut ctx.game else {
            bail!("running try_save_world_to_context without game context");
        };

        gctx.upate_world_description(self.mk_world())?;
        Ok(())
    }

    fn try_export_markdown(&self) -> Result<()> {
        let default_filename = if self.name.trim().is_empty() {
            "world.md".to_string()
        } else {
            format!("{}.md", self.name.replace(" ", "_"))
        };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(&default_filename)
            .add_filter("Markdown", &["md"])
            .save_file()
        else {
            return Ok(());
        };
        fs::write(path, self.to_markdown())?;
        Ok(())
    }

    fn to_markdown(&self) -> String {
        world_to_markdown(&self.mk_world())
    }

    fn begin_edit_character_name(&mut self, name: String) {
        self.editing_character_name = Some((name.clone(), name));
    }

    fn update_editing_character_name(&mut self, new_name: String) {
        if let Some((_, current_name)) = &mut self.editing_character_name {
            *current_name = new_name;
        }
    }

    fn finish_editing_character_name(&mut self) -> Result<()> {
        let Some((old_name, new_name)) = self.editing_character_name.take() else {
            return Ok(());
        };
        let new_name = new_name.trim().to_string();
        ensure!(!new_name.is_empty(), "Character name must not be empty");
        if new_name == old_name {
            return Ok(());
        }
        ensure!(
            !self.characters.contains_key(&new_name),
            "A character named {new_name} already exists"
        );
        let inputs = self
            .characters
            .remove(&old_name)
            .ok_or(eyre!("Character name invalid"))?;
        self.characters.insert(new_name, inputs);
        Ok(())
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
                self.characters.insert(name, CharacterInputs::default());
                cmd::none()
            }
            EditCharacterName(name) => {
                self.begin_edit_character_name(name);
                cmd::none()
            }
            DeleteCharacter(name) => cmd::transition(Modal::confirm(
                State::clone(self),
                format!("Do you really want to delete the character {name}?"),
                Some(MyMessage::ConfirmDeleteCharacter(name).into()),
                None,
            )),
            ConfirmDeleteCharacter(name) => {
                self.characters.remove(&name);
                cmd::none()
            }
            UpdateCharacterName(name) => {
                self.update_editing_character_name(name);
                cmd::none()
            }
            ConfirmCharacterNameEdit => {
                self.finish_editing_character_name()?;
                cmd::none()
            }
            UpdateCharacter(name, a) => {
                self.characters
                    .get_mut(&name)
                    .ok_or(eyre!("Character name invalid"))?
                    .description
                    .perform(a);
                cmd::none()
            }
            UpdateCharacterInitAction(name, a) => {
                self.characters
                    .get_mut(&name)
                    .ok_or(eyre!("Character name invalid"))?
                    .initial_action
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
            Button(which) => {
                let handler = self
                    .buttons
                    .get(&which)
                    .ok_or(eyre!("No such button: {which}"))?
                    .clone();
                handler(self, ctx)
            }
        }
    }

    fn view<'a>(&'a self, _ctx: &'a Context) -> iced::Element<'a, UiMessage> {
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

        let char_col =
            self.characters
                .iter()
                .map(|(name, content)| {
                    let name_row: iced::Element<'_, UiMessage> =
                        if matches!(&self.editing_character_name, Some((edited_name, _)) if edited_name == name)
                        {
                            let edited_name = &self
                                .editing_character_name
                                .as_ref()
                                .expect("checked above")
                                .1;
                            row![
                                text_input("Character name", edited_name)
                                    .on_input(|n| MyMessage::UpdateCharacterName(n).into())
                                    .on_submit(MyMessage::ConfirmCharacterNameEdit.into()),
                                button("ok").on_press(MyMessage::ConfirmCharacterNameEdit.into()),
                                button("delete")
                                    .on_press(MyMessage::DeleteCharacter(name.clone()).into()),
                            ]
                            .spacing(10)
                            .width(Length::Fill)
                            .into()
                        } else {
                            row![
                                text(name)
                                    .font(Font {
                                        weight: iced::font::Weight::Semibold,
                                        ..Font::DEFAULT
                                    })
                                    .size(16),
                                space::horizontal(),
                                button("✎")
                                    .on_press(MyMessage::EditCharacterName(name.clone()).into()),
                                button("delete")
                                    .on_press(MyMessage::DeleteCharacter(name.clone()).into()),
                            ]
                            .spacing(10)
                            .width(Length::Fill)
                            .into()
                        };
                    column![
                        name_row,
                        text_editor(&content.description)
                            .on_action(|a| MyMessage::UpdateCharacter(name.clone(), a).into()),
                        text("Initial Action:"),
                        text_editor(&content.initial_action).on_action(|a| {
                            MyMessage::UpdateCharacterInitAction(name.clone(), a).into()
                        }),
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

        let mut button_row = vec![space::horizontal().into()];
        for bcaption in self.buttons.keys() {
            button_row.push(
                button(text(bcaption))
                    .on_press(MyMessage::Button(bcaption.clone()).into())
                    .into(),
            );
        }

        button_row.push(space::horizontal().into());
        let content = container(
            scrollable(
                container(column(tlc).width(Length::Fill).spacing(20))
                    .padding(padding::all(10).right(20)),
            )
            .height(Length::Fill),
        )
        .style(|_theme| container::background(Color::from_rgb(0.95, 0.95, 0.95)));

        container(
            container(
                column![
                    content,
                    container(row(button_row).spacing(10).width(Length::Fill)).padding(10)
                ]
                .height(Length::Fill)
                .width(Length::Fill)
            )
            .padding(20)
            .max_width(800),
        )
        .center(Length::Fill)
        .into()
    }

    fn clone(&self) -> Box<dyn State> {
        Clone::clone(self).boxed()
    }
}

/// arc-new
fn an<F>(f: F) -> ActionFnArc
where
    F: Fn(&mut WorldEditor, &mut Context) -> Result<StateCommand> + Send + Sync + 'static,
{
    Arc::new(f)
}
