use std::collections::BTreeMap;

use color_eyre::{Result, eyre::eyre};
use iced::{
    Length, Task,
    widget::{button, column, container, radio, row, space, text, text_editor, text_input},
};
use strum::IntoEnumIterator;

use crate::{
    TryIntoExt, bold_default_font, bold_text,
    context::{Config, StyleKey},
    elem_list,
    message::ui_messages::OptionsMenu as MyMessage,
    save_config,
    state::{MainMenu, Modal, State, cmd},
    top_level_container,
};
use engine::{
    image_model::{self, Model, ModelStyle},
    llm,
};

#[derive(Debug, Clone, Default)]
struct StyleEntry {
    prefix: text_editor::Content,
    postfix: text_editor::Content,
}

#[derive(Debug, Clone)]
pub struct OptionsMenu {
    styles: BTreeMap<(Model, String), StyleEntry>,
}

impl OptionsMenu {
    pub fn new(config: &Config) -> Result<Self> {
        let styles = config
            .styles
            .iter()
            .map(|(key, style)| {
                (
                    (key.model, key.name.clone()),
                    StyleEntry {
                        prefix: text_editor::Content::with_text(&style.prefix),
                        postfix: text_editor::Content::with_text(&style.postfix),
                    },
                )
            })
            .collect();
        Ok(Self { styles })
    }

    fn get_style_enty(&mut self, i: usize) -> Result<(Model, &String, &mut StyleEntry)> {
        self.styles
            .iter_mut()
            .map(|((model, name), entry)| (*model, name, entry))
            .nth(i)
            .ok_or(eyre!("Invalid index"))
    }
}

impl State for OptionsMenu {
    fn update(
        &mut self,
        event: crate::message::UiMessage,
        ctx: &mut crate::context::Context,
    ) -> Result<crate::state::StateCommand> {
        let msg: MyMessage = event.try_into_ex()?;

        use MyMessage::*;
        match msg {
            LLMTokenChanged(provider, token) => {
                ctx.config.llm_tokens.insert(provider, token);
                cmd::none()
            }
            ImgModelTokenChanged(provider, val) => {
                ctx.config.img_model_tokens.insert(provider, val);
                cmd::none()
            }
            SelectImageModel(model) => {
                ctx.config.current_img_model = model;
                cmd::none()
            }
            SelectStyle(idx) => {
                let (model, name, _) = self.get_style_enty(idx)?;
                ctx.config.active_model_style.insert(model, name.clone());
                cmd::none()
            }
            EditStylePrefix(i, action) => {
                let (model, _, entry) = self.get_style_enty(i)?;
                entry.prefix.perform(action);
                ctx.config
                    .active_style_for_mut(model)
                    .ok_or(eyre!("There is no active style for the model"))?
                    .prefix = entry.prefix.text();
                cmd::none()
            }
            EditStylePostfix(i, action) => {
                let (model, _, entry) = self.get_style_enty(i)?;
                entry.postfix.perform(action);
                ctx.config
                    .active_style_for_mut(model)
                    .ok_or(eyre!("There is no active style for the model"))?
                    .postfix = entry.postfix.text();
                cmd::none()
            }
            NewStyle(model, name) => {
                ctx.config.styles.insert(
                    StyleKey {
                        model,
                        name: name.clone(),
                    },
                    ModelStyle::default(),
                );
                ctx.config.active_model_style.insert(model, name.clone());
                self.styles.insert((model, name), StyleEntry::default());
                cmd::none()
            }
            Ok => {
                save_config(&ctx.config)?;
                if let Some(gctx) = &mut ctx.game {
                    gctx.game.imgmod = ctx.config.get_image_model()?;
                    gctx.game.img_style = ctx.config.active_style().cloned();
                    gctx.game.llm = ctx.config.get_llm()?;
                }
                cmd::transition(MainMenu::try_new()?)
            }
            AddModelStyleButton(model) => cmd::transition(Modal::input(
                State::clone(self),
                "New Style",
                "name...",
                move |name| Task::done(NewStyle(model, name).into()),
            )),
            UnselectStyle(model) => {
                ctx.config.active_model_style.remove(&model);
                cmd::none()
            }
            SelectLLM(provided_model) => {
                ctx.config.current_llm = provided_model;
                cmd::none()
            }
        }
    }

    fn view<'a>(
        &'a self,
        ctx: &'a crate::context::Context,
    ) -> iced::Element<'a, crate::message::UiMessage> {
        let mut items = Vec::from(elem_list![
            bold_text("Options").size(26).width(Length::Fill).center(),
            space().height(20),
            bold_text("LLM Provider API Keys").size(22),
        ]);

        for provider in llm::ModelProvider::iter() {
            let value = ctx
                .config
                .llm_tokens
                .get(&provider)
                .map(String::as_str)
                .unwrap_or("");

            items.push(text(format!("{provider}")).into());
            items.push(
                text_input("API token", value)
                    .on_input(move |s| MyMessage::LLMTokenChanged(provider, s).into())
                    .width(Length::Fill)
                    .into(),
            );
        }

        items.extend(elem_list![
            space().height(20),
            bold_text("Active LLM").size(22),
            column(llm::ProvidedModel::iter().map(|m| {
                radio(format!("{m}"), m, Some(ctx.config.current_llm), |m| {
                    MyMessage::SelectLLM(m).into()
                })
                .into()
            }))
            .spacing(10),
            space().height(20),
            bold_text("Active Image Model").size(22),
            column(image_model::ProvidedModel::iter().map(|m| {
                radio(format!("{m}"), m, Some(ctx.config.current_img_model), |m| {
                    MyMessage::SelectImageModel(m).into()
                })
                .into()
            }))
            .spacing(10),
            space().height(20),
            bold_text("Image Model API Keys").size(22)
        ]);

        for provider in image_model::ModelProvider::iter() {
            let value = ctx
                .config
                .img_model_tokens
                .get(&provider)
                .map(String::as_str)
                .unwrap_or("");

            items.push(text(format!("{provider}")).into());
            items.push(
                text_input("API token", value)
                    .on_input(move |s| MyMessage::ImgModelTokenChanged(provider, s).into())
                    .width(Length::Fill)
                    .into(),
            );
        }

        items.push(space().height(30).into());
        items.push(bold_text("Image Styles").size(22).into());
        items.push(space().height(10).into());

        for model in image_model::Model::iter() {
            let styles = ctx
                .config
                .styles
                .keys()
                .enumerate()
                .filter(|(_, key)| key.model == model);
            let active_style = ctx.config.active_model_style.get(&model).and_then(|name| {
                ctx.config.style_key_to_idx(&StyleKey {
                    model,
                    name: name.clone(),
                })
            });

            // the value that idetifies the selected style is Option<usize>,
            // which is the type of active_style, but radios Third argument
            // is Option<ValueType>, so it's wrapped in an additional some
            items.extend(elem_list![
                row![
                    text!("{model}").font(bold_default_font()),
                    space::horizontal(),
                    button("Add Style").on_press(MyMessage::AddModelStyleButton(model).into())
                ],
                radio("No Style", None, Some(active_style), |_| {
                    MyMessage::UnselectStyle(model).into()
                })
            ]);

            for (i, key) in styles {
                items.push(
                    radio(&key.name, Some(i), Some(active_style), |i| {
                        MyMessage::SelectStyle(i.unwrap()).into()
                    })
                    .into(),
                );

                if active_style == Some(i) {
                    items.push(
                        container(
                            column![
                                text("Prefix"),
                                text_editor(&self.styles[&(key.model, key.name.clone())].prefix)
                                    .on_action(move |a| MyMessage::EditStylePrefix(i, a).into()),
                                text("Postfix"),
                                text_editor(&self.styles[&(key.model, key.name.clone())].postfix)
                                    .on_action(move |a| MyMessage::EditStylePostfix(i, a).into()),
                            ]
                            .spacing(10),
                        )
                        .padding(50)
                        .into(),
                    );
                }
            }
        }

        items.push(space().height(30).into());
        items.push(row![button("Ok").on_press(MyMessage::Ok.into())].into());

        top_level_container(
            column(items)
                .spacing(12)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .into()
    }

    fn clone(&self) -> Box<dyn State> {
        Box::new(Clone::clone(self))
    }
}
