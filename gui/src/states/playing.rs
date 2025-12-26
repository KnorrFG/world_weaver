use std::mem;

use color_eyre::{Result, eyre::bail};
use engine::game::{AdvanceResult, Image, StartResultOrData, TurnInput, TurnOutput};
use iced::{
    Length, Task, Theme, padding,
    widget::{
        self, Button, Column, button, container, image::Handle, markdown, row, scrollable, space,
        text_editor,
    },
};
use nonempty::nonempty;

use crate::{Context, Message, State, StringError, cmd};

#[derive(Debug)]
pub struct Playing {
    /// whether we're currently expecting incoming streaming data
    sub_state: SubState,
    current_output: String,
    markdown: Vec<markdown::Item>,
    action_text_content: text_editor::Content,
    image_data: Option<(iced::advanced::image::Handle, String)>,
}

impl Playing {
    pub fn new() -> Self {
        Self {
            sub_state: SubState::Uninit,
            current_output: "".into(),
            action_text_content: text_editor::Content::default(),
            markdown: vec![],
            image_data: None,
        }
    }

    fn complete_turn(
        &mut self,
        ctx: &mut Context,
        input: TurnInput,
        output: TurnOutput,
        image: Image,
    ) -> Result<()> {
        let id = ctx.save.append_image(&image.jpeg_bytes)?;
        ctx.game.update(
            input,
            output.clone(),
            nonempty![id],
            nonempty![image.caption],
        )?;
        ctx.save.write_game_data(ctx.game.get_data())?;
        self.sub_state = SubState::Complete(output);
        Ok(())
    }
}

#[derive(Debug, Default)]
enum SubState {
    #[default]
    Uninit,
    Complete(TurnOutput),
    WaitingForOutput {
        input: TurnInput,
        output: Option<TurnOutput>,
        image: Option<Image>,
    },
}

impl State for Playing {
    fn update(
        &mut self,
        message: Message,
        ctx: &mut crate::Context,
    ) -> color_eyre::eyre::Result<crate::StateCommand> {
        match message {
            Message::OutputComplete(turn_output) => {
                let output = turn_output?;

                let SubState::WaitingForOutput {
                    input,
                    output: _,
                    image,
                } = mem::take(&mut self.sub_state)
                else {
                    bail!("Not in WaitingForOutput substate when receiving OutputComplete");
                };

                if let Some(image) = image {
                    self.complete_turn(ctx, input, output, image)?;
                } else {
                    self.sub_state = SubState::WaitingForOutput {
                        input,
                        output: Some(output),
                        image: None,
                    };
                }

                cmd::none()
            }
            Message::NewTextFragment(t) => {
                self.current_output.push_str(&t?);
                self.markdown = markdown::parse(&self.current_output).collect();
                cmd::none()
            }
            Message::Init => match ctx.game.start_or_get_last_output() {
                StartResultOrData::StartResult(
                    AdvanceResult {
                        text_stream,
                        round_output,
                        image,
                    },
                    input,
                ) => {
                    let output_fut = Task::perform(round_output, |res| {
                        Message::OutputComplete(res.map_err(StringError::from))
                    });
                    let image_fut = Task::perform(image, |res| {
                        Message::ImageReady(res.map_err(StringError::from))
                    });
                    let stream_task = Task::run(text_stream, |res| {
                        Message::NewTextFragment(res.map_err(StringError::from))
                    });
                    self.sub_state = SubState::WaitingForOutput {
                        input,
                        output: None,
                        image: None,
                    };
                    self.current_output = String::new();
                    cmd::task(Task::batch([output_fut, stream_task, image_fut]))
                }
                StartResultOrData::Data(turn_data) => {
                    self.current_output = turn_data.output.text.clone();
                    self.sub_state = SubState::Complete(turn_data.output);
                    self.image_data = Some((
                        Handle::from_bytes(ctx.save.read_image(*turn_data.image_ids.first())?),
                        turn_data.image_captions.first().clone(),
                    ));
                    self.markdown = markdown::parse(&self.current_output).collect();
                    cmd::none()
                }
            },
            Message::UpdateActionText(action) => {
                self.action_text_content.perform(action);
                cmd::none()
            }
            Message::ProposedActionButtonPressed(s) => {
                if self.action_text_content.text() == s {
                    cmd::task(Task::done(Message::Submit))
                } else {
                    self.action_text_content = text_editor::Content::with_text(&s);
                    cmd::none()
                }
            }
            Message::Submit => {
                let input = TurnInput::PlayerAction(self.action_text_content.text());
                self.current_output.clear();
                let AdvanceResult {
                    text_stream,
                    round_output,
                    image,
                } = ctx.game.send_to_llm(input.clone());
                self.sub_state = SubState::WaitingForOutput {
                    input,
                    output: None,
                    image: None,
                };
                cmd::task(Task::batch([
                    Task::perform(round_output, |x| {
                        Message::OutputComplete(x.map_err(StringError::from))
                    }),
                    Task::perform(image, |x| Message::ImageReady(x.map_err(StringError::from))),
                    Task::run(text_stream, |x| {
                        Message::NewTextFragment(x.map_err(StringError::from))
                    }),
                ]))
            }
            Message::ImageReady(image) => {
                let img = image?;
                self.image_data = Some((
                    Handle::from_bytes(img.jpeg_bytes.clone()),
                    img.caption.clone(),
                ));

                let SubState::WaitingForOutput {
                    input,
                    output,
                    image: _,
                } = mem::take(&mut self.sub_state)
                else {
                    bail!("Not in WaitingForOutput substate when receiving ImageReady");
                };

                if let Some(output) = output {
                    self.complete_turn(ctx, input, output, img)?;
                } else {
                    self.sub_state = SubState::WaitingForOutput {
                        input,
                        output: None,
                        image: Some(img),
                    };
                }

                cmd::none()
            }
        }
    }

    fn view<'a>(&'a self, _ctx: &'a crate::Context) -> iced::Element<'a, Message> {
        let side_bar_width = 400;
        let mut sidebar = Column::new();
        if let Some((handle, caption)) = &self.image_data {
            sidebar = sidebar.push(widget::column![
                widget::image(handle).width(side_bar_width),
                widget::text(caption).center(),
            ]);
        };
        if let SubState::Complete(output) = &self.sub_state {
            sidebar = sidebar
                .extend([
                    proposed_action_button(&output.proposed_next_actions[0])
                        .width(side_bar_width)
                        .into(),
                    proposed_action_button(&output.proposed_next_actions[1])
                        .width(side_bar_width)
                        .into(),
                    proposed_action_button(&output.proposed_next_actions[2])
                        .width(side_bar_width)
                        .into(),
                    text_editor(&self.action_text_content)
                        .placeholder("Type an action")
                        .on_action(Message::UpdateActionText)
                        .width(side_bar_width)
                        .into(),
                    row![space::horizontal(), button("Go").on_press(Message::Submit)]
                        .width(side_bar_width)
                        .into(),
                ])
                .spacing(10)
                .into();
        }

        let main_row = row![
            scrollable(
                container(
                    markdown::view(&self.markdown, Theme::TokyoNight).map(|_| unreachable!())
                )
                .padding(padding::all(10.).right(20.))
                .max_width(600)
            ),
            container(sidebar).padding(20)
        ];

        container(main_row).center_x(Length::Fill).into()
    }
}

fn proposed_action_button<'a>(text: &'a str) -> Button<'a, Message> {
    button(text).on_press(Message::ProposedActionButtonPressed(text.into()))
}
