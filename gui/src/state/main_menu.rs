use crate::{State, message::UiMessage, context::Context, state::StateCommand};

#[derive(Debug)]
pub struct MainMenu;

impl State for MainMenu {
    fn update(
        &mut self,
        event: UiMessage,
        ctx: &mut Context,
    ) -> color_eyre::eyre::Result<StateCommand> {
        todo!()
    }

    fn view<'a>(&'a self, ctx: &'a Context) -> iced::Element<'a, crate::message::UiMessage> {
        todo!()
    }

    fn clone(&self) -> Box<dyn State> {
        todo!()
    }
}
