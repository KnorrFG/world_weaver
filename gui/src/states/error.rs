use iced::{
    Border, Color, Element, Length,
    alignment::Horizontal,
    widget::{self, button, column, container, row, scrollable, space, stack, text},
};

use crate::{Message, State, cmd};

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub parent_state: Option<Box<dyn State>>,
}

impl Clone for Error {
    fn clone(&self) -> Self {
        Self {
            message: self.message.clone(),
            parent_state: self.parent_state.as_ref().map(|p| p.clone()),
        }
    }
}

impl State for Error {
    fn update(
        &mut self,
        event: crate::Message,
        _ctx: &mut crate::Context,
    ) -> color_eyre::eyre::Result<crate::StateCommand> {
        if let Some(parent) = &self.parent_state
            && let Message::ErrorConfirmed = event
        {
            cmd::transition(parent.clone())
        } else {
            cmd::none()
        }
    }

    fn view<'a>(&'a self, ctx: &'a crate::Context) -> iced::Element<'a, crate::Message> {
        let displayed_error = widget::container(
            column![
                text("Error:"),
                container(scrollable(text(&self.message))).padding(20),
                container(button("Ok").on_press(Message::ErrorConfirmed)).align_right(Length::Fill)
            ]
            .width(Length::Shrink)
            .spacing(10),
        )
        .padding(20)
        .style(|theme| widget::container::secondary(theme).border(Border::default()));

        let mut children = vec![];
        if let Some(parent) = &self.parent_state {
            children.push(parent.view(ctx));
        }

        children.push(dim_layer());
        children.push(container(displayed_error).center(Length::Fill).into());
        stack(children).into()
    }

    fn clone(&self) -> Box<dyn State> {
        Box::new(Clone::clone(self))
    }
}

fn dim_layer() -> Element<'static, Message> {
    widget::Container::new(widget::space())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| {
            widget::container::Style::default().background(Color::from_rgba(0., 0., 0., 0.1))
        })
        .into()
}
