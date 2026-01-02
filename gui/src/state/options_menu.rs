use color_eyre::Result;
use iced::{
    Length,
    widget::{button, column, radio, row, space, text, text_input},
};
use strum::IntoEnumIterator;

use crate::{
    TryIntoExt, bold_text, elem_list,
    message::ui_messages::OptionsMenu as MyMessage,
    save_config,
    state::{MainMenu, State, cmd},
    top_level_container,
};
use engine::image_model;

#[derive(Debug, Clone)]
pub struct OptionsMenu;

impl State for OptionsMenu {
    fn update(
        &mut self,
        event: crate::message::UiMessage,
        ctx: &mut crate::context::Context,
    ) -> Result<crate::state::StateCommand> {
        let msg: MyMessage = event.try_into_ex()?;

        use MyMessage::*;
        match msg {
            ClaudeTokenChanged(val) => {
                ctx.config.claude_token = val;
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

            Ok => {
                save_config(&ctx.config)?;
                cmd::transition(MainMenu::try_new()?)
            }
        }
    }

    fn view<'a>(
        &'a self,
        ctx: &'a crate::context::Context,
    ) -> iced::Element<'a, crate::message::UiMessage> {
        let mut items = Vec::from(elem_list![
            bold_text("Options").width(Length::Fill).center(),
            space().height(20),
            text("Anthropic (Claude) API Key"),
            text_input("sk-ant-...", &ctx.config.claude_token,)
                .on_input(|s| MyMessage::ClaudeTokenChanged(s).into())
                .width(Length::Fill),
            space().height(20),
            text("Active Image Model"),
            column(image_model::Model::iter().map(|m| {
                radio(
                    format!("{m} ({})", m.provider()),
                    m,
                    Some(ctx.config.current_img_model),
                    |m| MyMessage::SelectImageModel(m).into(),
                )
                .into()
            }))
            .spacing(10),
            space().height(20),
            bold_text("Image Model API Keys"),
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
