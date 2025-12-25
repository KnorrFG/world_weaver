use crate::State;

#[derive(Debug)]
pub struct MainMenu;

impl State for MainMenu {
    fn update(
        &mut self,
        event: crate::Message,
        ctx: &mut crate::Context,
    ) -> color_eyre::eyre::Result<crate::StateCommand> {
        todo!()
    }

    fn render<'a>(&'a self, ctx: &crate::Context) -> iced::Element<'a, crate::Message> {
        todo!()
    }
}
