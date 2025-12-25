use std::mem;

use color_eyre::{eyre::bail, owo_colors::OwoColorize};
use engine::game::{AdvanceResult, StartResultOrOutput, TurnInput, TurnOutput};
use iced::{
    Color, Element, Length, Padding, Task, Theme, padding,
    widget::{
        Button, Column, Space, button, column, container, markdown, row, scrollable, space, text,
        text_editor, text_input,
    },
};

use crate::{Message, State, StringError, cmd, save_json_file};

#[derive(Debug)]
pub struct Playing {
    /// whether we're currently expecting incoming streaming data
    sub_state: SubState,
    current_output: String,
    markdown: Vec<markdown::Item>,
    action_text_content: text_editor::Content,
}

impl Playing {
    pub fn new() -> Self {
        Self {
            sub_state: SubState::Uninit,
            current_output: "".into(),
            action_text_content: text_editor::Content::default(),
            markdown: vec![],
        }
    }
}

#[derive(Debug)]
enum SubState {
    Uninit,
    Complete(TurnOutput),
    WaitingForOutput(TurnInput),
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
                let mut state = SubState::Complete(output.clone());
                mem::swap(&mut self.sub_state, &mut state);

                let SubState::WaitingForOutput(input) = state else {
                    bail!("Not in WaitingForOutput substate when receiving OutputComplete");
                };

                ctx.game.update(input, output)?;
                save_json_file(&ctx.save_path, ctx.game.get_data())?;
                cmd::none()
            }
            Message::NewTextFragment(t) => {
                self.current_output.push_str(&t?);
                self.markdown = markdown::parse(&self.current_output).collect();
                cmd::none()
            }
            Message::Init => match ctx.game.start_or_get_last_output() {
                StartResultOrOutput::StartResult(
                    AdvanceResult {
                        text_stream,
                        round_output,
                    },
                    input,
                ) => {
                    let fut_task = Task::perform(round_output, |res| {
                        Message::OutputComplete(res.map_err(StringError::from))
                    });
                    let stream_task = Task::run(text_stream, |res| {
                        Message::NewTextFragment(res.map_err(StringError::from))
                    });
                    self.sub_state = SubState::WaitingForOutput(input);
                    self.current_output = String::new();
                    cmd::task(Task::batch([fut_task, stream_task]))
                }
                StartResultOrOutput::Output(output) => {
                    self.current_output = output.text.clone();
                    self.sub_state = SubState::Complete(output);
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
                } = ctx.game.send_to_llm(input.clone());
                self.sub_state = SubState::WaitingForOutput(input);
                cmd::task(Task::batch([
                    Task::perform(round_output, |x| {
                        Message::OutputComplete(x.map_err(StringError::from))
                    }),
                    Task::run(text_stream, |x| {
                        Message::NewTextFragment(x.map_err(StringError::from))
                    }),
                ]))
            }
        }
    }

    fn view<'a>(&'a self, ctx: &'a crate::Context) -> iced::Element<'a, Message> {
        let side_bar_width = 400;
        let sidebar = Column::new();
        let sidebar: Element<Message> = if let SubState::Complete(output) = &self.sub_state {
            sidebar
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
                .into()
        } else {
            space().width(side_bar_width).into()
        };

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
