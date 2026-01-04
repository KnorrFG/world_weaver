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
    top_level_container, worlds_dir,
};

use color_eyre::{
    Result,
    eyre::{bail, ensure, eyre},
};
use engine::game::WorldDescription;
use iced::{
    Font, Length, Task,
    widget::{Space, button, column, container, row, rule, space, text, text_editor, text_input},
};

use super::State;

type ActionFnArc = Arc<dyn Fn(&mut WorldEditor, &mut Context) -> Result<StateCommand>>;

#[derive(Clone)]
pub struct WorldEditor {
    name: String,
    description: text_editor::Content,
    init_action: text_editor::Content,
    characters: BTreeMap<String, text_editor::Content>,
    on_abort: Option<ActionFnArc>,
    on_save: Option<ActionFnArc>,
    on_save_and_play: Option<ActionFnArc>,
}

impl fmt::Debug for WorldEditor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldEditor")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("init_action", &self.init_action)
            .field("characters", &self.characters)
            .field("on_abort", &self.on_abort.as_ref().map(|_| "..."))
            .field("on_save", &self.on_save.as_ref().map(|_| "..."))
            .field(
                "on_save_and_play",
                &self.on_save_and_play.as_ref().map(|_| "..."),
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
                .map(|(k, v)| (k.clone(), text_editor::Content::with_text(v)))
                .collect(),
            on_abort: san(|_, _| cmd::transition(MainMenu::try_new()?)),
            on_save: san(|this, ctx| {
                this.try_save_world_to_context(ctx)?;
                cmd::transition(Modal::message(
                    State::clone(this),
                    "Info",
                    "Saving succesful",
                ))
            }),
            on_save_and_play: san(|this, ctx| {
                this.try_save_world_to_context(ctx)?;
                cmd::transition(Playing::new())
            }),
        }
    }

    pub fn for_worlds_menu() -> Self {
        Self {
            name: "".into(),
            description: text_editor::Content::default(),
            init_action: text_editor::Content::default(),
            characters: BTreeMap::new(),
            on_abort: san(|_, _| cmd::transition(WorldMenu::try_new()?)),
            on_save: san(|this, _| {
                this.try_save_world()?;
                cmd::transition(Modal::message(
                    State::clone(this),
                    "Info",
                    "Saving succesful",
                ))
            }),
            on_save_and_play: san(|this, _| {
                let world = this.try_save_world()?;
                cmd::transition(StartNewGame::new(world))
            }),
        }
    }

    fn try_save_world(&self) -> Result<WorldDescription> {
        let path = self.current_save_path()?;
        ensure!(!path.exists(), "A world with that name alread exists");
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
                .map(|(k, v)| (k.clone(), v.text()))
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
            Save => (self
                .on_save
                .clone()
                .ok_or(eyre!("Save called but no on_save"))?)(self, ctx),
            SaveAndPlay => (self
                .on_save_and_play
                .clone()
                .ok_or(eyre!("SaveAndPlay called but no on_save_and_play"))?)(
                self, ctx
            ),
            Abort => (self
                .on_abort
                .clone()
                .ok_or(eyre!("Abort called but no on_abort"))?)(self, ctx),
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

        let mut button_row = vec![space::horizontal().into()];
        if self.on_abort.is_some() {
            button_row.push(button("Abort").on_press(MyMessage::Abort.into()).into());
        }
        if self.on_save.is_some() {
            button_row.push(button("Save").on_press(MyMessage::Save.into()).into());
        }
        if self.on_save_and_play.is_some() {
            button_row.push(
                button("Save and Play")
                    .on_press(MyMessage::SaveAndPlay.into())
                    .into(),
            );
        }
        button_row.push(space::horizontal().into());
        tlc.push(row(button_row).spacing(10).width(Length::Fill).into());

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

/// some-arc-new
fn san<F>(f: F) -> Option<ActionFnArc>
where
    F: Fn(&mut WorldEditor, &mut Context) -> Result<StateCommand> + Send + Sync + 'static,
{
    Some(Arc::new(f))
}
