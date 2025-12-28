use std::mem;

use color_eyre::{
    Result,
    eyre::{bail, ensure, eyre},
};
use engine::game::{AdvanceResult, Image, StartResultOrData, TurnData, TurnInput, TurnOutput};
use iced::{
    Background, Color, Element, Length, Task, Theme,
    alignment::Horizontal,
    padding,
    widget::{
        self, Button, Column, button, container,
        image::Handle,
        markdown, row, scrollable, space,
        text_editor::{self, Edit},
        text_input,
    },
};
use nonempty::nonempty;

use crate::{Context, Message, State, StateCommand, StringError, cmd, elem_list};

#[derive(Debug)]
pub struct Playing {
    sub_state: SubState,
    current_output: String,
    goto_turn_input: Option<usize>,
    markdown: Vec<markdown::Item>,
    action_text_content: text_editor::Content,
    gm_instruction_text_content: text_editor::Content,
    image_data: Option<(iced::advanced::image::Handle, String)>,
}

enum EditorId {
    PlayerAction,
    GMInstruction,
}

impl Playing {
    pub fn new() -> Self {
        Self {
            sub_state: SubState::Uninit,
            current_output: "".into(),
            goto_turn_input: None,
            action_text_content: text_editor::Content::default(),
            gm_instruction_text_content: text_editor::Content::default(),
            markdown: vec![],
            image_data: None,
        }
    }

    fn request_summary(
        &mut self,
        ctx: &mut Context,
        input: TurnInput,
        output: TurnOutput,
        image: Image,
    ) -> Result<StateCommand> {
        self.sub_state = SubState::WaitingForSummary {
            input,
            output,
            image,
        };
        let fut = ctx.game.mk_summary_if_neccessary();
        cmd::task(Task::perform(fut, |res| {
            Message::SummaryFinished(res.map_err(StringError::from))
        }))
    }

    fn complete_turn(
        &mut self,
        ctx: &mut Context,
        input: TurnInput,
        output: TurnOutput,
        image: Image,
        summary: Option<String>,
    ) -> Result<()> {
        let id = ctx.save.append_image(&image.jpeg_bytes)?;
        ctx.game.update(
            input,
            output.clone(),
            nonempty![id],
            nonempty![image.caption],
            summary,
        )?;
        ctx.save.write_game_data(ctx.game.get_data())?;
        self.sub_state = SubState::Complete(output);
        self.action_text_content = text_editor::Content::default();
        self.gm_instruction_text_content = text_editor::Content::default();
        Ok(())
    }

    fn update_editor_content(
        &mut self,
        action: text_editor::Action,
        editor: EditorId,
    ) -> Result<StateCommand, color_eyre::eyre::Error> {
        if let text_editor::Action::Edit(Edit::Enter) = action {
            cmd::task(Task::done(Message::Submit))
        } else {
            match editor {
                EditorId::PlayerAction => self.action_text_content.perform(action),
                EditorId::GMInstruction => self.gm_instruction_text_content.perform(action),
            }
            cmd::none()
        }
    }

    /// loading completed turn n actually means loading turn n+1, but this way it's less confusing
    fn load_completed_turn(&mut self, ctx: &mut Context, target_turn: usize) -> Result<()> {
        let turn_data = ctx
            .game
            .get_data()
            .turn_data
            .get(target_turn)
            .ok_or(eyre!("Invalid target turn: {target_turn}"))?;
        self.image_data = Some((
            Handle::from_bytes(ctx.save.read_image(*turn_data.image_ids.first())?),
            turn_data.image_captions.first().clone(),
        ));
        self.markdown = markdown::parse(&turn_data.output.text).collect();

        // this looks wrong but is right. If we load the completed turn 0, the displayed output
        // is the ouput of turn 0, but that means we're actually in turn 1
        if target_turn + 1 == ctx.game.current_turn() {
            self.sub_state = SubState::Complete(turn_data.output.clone());
        } else {
            self.sub_state = SubState::InThePast {
                completed_turn: target_turn,
                _data: turn_data.clone(),
            };
        }
        Ok(())
    }

    /// turn semantics are as follows:
    /// when the game starts, that's turn 0, before there is any input or output
    /// the result of the 0th turn is stored in game.data_turn_data[0].
    /// As soon as you finish the first turn (index 0), you are in turn 1.
    /// But in turn 1, you do see the outputs of turn 0;
    fn current_turn(&self, ctx: &Context) -> usize {
        match &self.sub_state {
            SubState::InThePast {
                completed_turn,
                _data,
            } => *completed_turn + 1,
            _ => ctx.game.current_turn(),
        }
    }

    fn goto_turn_string(&self) -> String {
        self.goto_turn_input
            .as_ref()
            .map(|x| x.to_string())
            .unwrap_or("".into())
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
    WaitingForSummary {
        input: TurnInput,
        output: TurnOutput,
        image: Image,
    },
    InThePast {
        completed_turn: usize,
        _data: TurnData,
    },
}

impl SubState {
    fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
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
                    self.request_summary(ctx, input, output, image)
                } else {
                    self.sub_state = SubState::WaitingForOutput {
                        input,
                        output: Some(output),
                        image: None,
                    };
                    cmd::none()
                }
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
                self.update_editor_content(action, EditorId::PlayerAction)
            }
            Message::UpdateGMInstructionText(action) => {
                self.update_editor_content(action, EditorId::GMInstruction)
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
                // let input = TurnInput::player_action(self.action_text_content.text());
                let input = TurnInput {
                    player_action: self.action_text_content.text(),
                    gm_instruction: self.gm_instruction_text_content.text(),
                };
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
                    self.request_summary(ctx, input, output, img)
                } else {
                    self.sub_state = SubState::WaitingForOutput {
                        input,
                        output: None,
                        image: Some(img),
                    };
                    cmd::none()
                }
            }
            Message::SummaryFinished(output_message) => {
                let SubState::WaitingForSummary {
                    input,
                    output,
                    image,
                } = mem::take(&mut self.sub_state)
                else {
                    bail!("Not in Waiting For Summary while receiving SummaryFinished");
                };
                self.complete_turn(ctx, input, output, image, output_message?.map(|m| m.text))?;
                cmd::none()
            }
            Message::PrevTurnButtonPressed => {
                let target_turn = match &self.sub_state {
                    SubState::Complete(_) => ctx.game.current_turn() - 2,
                    SubState::InThePast {
                        completed_turn: turn,
                        ..
                    } => *turn - 1,
                    other => bail!(
                        "PrevTurnButtonPressed but Substate is not Complete or InThePast: {:?}",
                        other
                    ),
                };
                self.load_completed_turn(ctx, target_turn)?;
                cmd::none()
            }
            Message::NextTurnButtonPressed => {
                let target_turn = match &self.sub_state {
                    SubState::InThePast {
                        completed_turn: turn,
                        ..
                    } => *turn + 1,
                    other => bail!(
                        "PrevTurnButtonPressed but Substate is not Complete or InThePast: {:?}",
                        other
                    ),
                };
                self.load_completed_turn(ctx, target_turn)?;
                cmd::none()
            }
            Message::UpdateTurnInput(inp) => {
                self.goto_turn_input = inp.parse().ok();
                cmd::none()
            }
            Message::GotoTurnPressed => {
                if let Some(target) = self.goto_turn_input {
                    ensure!(
                        (1..=ctx.game.current_turn()).contains(&target),
                        "Invalid turn number"
                    );
                    self.load_completed_turn(ctx, target - 1)?;
                }
                cmd::none()
            }
            Message::GoToCurrentTurn => {
                self.load_completed_turn(ctx, ctx.game.current_turn() - 1)?;
                cmd::none()
            }
        }
    }

    fn view<'a>(&'a self, ctx: &'a crate::Context) -> iced::Element<'a, Message> {
        let mut sidebar = Column::new();
        if let Some((handle, caption)) = &self.image_data {
            sidebar = sidebar.extend([
                container(widget::image(handle).height(Length::Fill))
                    .max_width(800)
                    .into(),
                widget::text(caption).into(),
            ]);
        };

        let mut main_col = widget::column![
            markdown::view(&self.markdown, Theme::TokyoNight).map(|_| unreachable!())
        ];

        let button_w = 500;
        let elems: Vec<_> = match &self.sub_state {
            SubState::Complete(output) => mk_input_ui_portion(
                output,
                button_w,
                &self.action_text_content,
                &self.gm_instruction_text_content,
            )
            .into_iter()
            .chain([
                widget::rule::horizontal(1).into(),
                mk_turn_selection_buttons(ctx, ctx.game.current_turn(), &self.goto_turn_string())
                    .into(),
            ])
            .collect(),
            SubState::InThePast {
                completed_turn: turn,
                _data,
            } => {
                vec![
                    mk_turn_selection_buttons(ctx, *turn, &self.goto_turn_string()).into(),
                    button("Goto current turn")
                        .on_press(Message::GoToCurrentTurn)
                        .into(),
                ]
            }
            _ => vec![],
        };

        main_col = main_col
            .push(
                widget::column(elems)
                    .max_width(500)
                    .spacing(15)
                    .align_x(Horizontal::Center),
            )
            .spacing(10);

        let text_row = row![
            container(scrollable(
                container(main_col.align_x(Horizontal::Center))
                    .padding(padding::all(10.).right(20.))
            ))
            .width(700)
            .padding(10)
            .style(
                |_theme| container::background(Background::Color(Color::from_rgb(
                    0.95, 0.95, 0.95
                )))
            ),
            sidebar.align_x(Horizontal::Center).height(Length::Fill)
        ]
        .spacing(20);

        let main_col = widget::column![
            widget::text!(
                "{} - Turn {}",
                ctx.game.world_name(),
                self.current_turn(ctx),
            )
            .size(32),
            widget::rule::horizontal(2),
            container(text_row).center_x(Length::Fill).padding(20)
        ]
        .align_x(Horizontal::Center)
        .max_width(1500)
        .spacing(10);

        Element::from(
            container(main_col)
                .center_x(Length::Fill)
                .padding(padding::top(20)),
        )
        // .explain(iced::Color::from_rgb(1., 0., 0.))
    }
}

fn proposed_action_button<'a>(text: &'a str) -> Button<'a, Message> {
    button(text).on_press(Message::ProposedActionButtonPressed(text.into()))
}

fn mk_turn_selection_buttons<'a>(
    ctx: &'a Context,
    current_turn: usize,
    goto_turn_input: &str,
) -> impl Into<Element<'a, Message>> {
    let mut row = widget::Row::new();
    if current_turn > 0 {
        row = row.push(widget::button("←").on_press(Message::PrevTurnButtonPressed));
    }
    row = row.push(widget::space::horizontal());
    row = row.push(
        text_input("turn", goto_turn_input)
            .on_input(Message::UpdateTurnInput)
            .on_submit(Message::GotoTurnPressed),
    );
    row = row.push(widget::button("Goto Turn").on_press(Message::GotoTurnPressed));
    row = row.push(widget::space::horizontal());
    if current_turn < ctx.game.current_turn() - 1 {
        row = row.push(widget::button("→").on_press(Message::NextTurnButtonPressed));
    }

    row
}

fn mk_input_ui_portion<'a>(
    output: &'a TurnOutput,
    button_w: u32,
    action_text_content: &'a text_editor::Content,
    gm_instruction_text_content: &'a text_editor::Content,
) -> impl IntoIterator<Item = Element<'a, Message>> {
    elem_list![
        widget::Space::new().height(20),
        proposed_action_button(&output.proposed_next_actions[0]).width(button_w),
        proposed_action_button(&output.proposed_next_actions[1]).width(button_w),
        proposed_action_button(&output.proposed_next_actions[2]).width(button_w),
        widget::Space::new().height(10),
        row![widget::text("What to do next:"), space::horizontal()],
        widget::text_editor(action_text_content)
            .placeholder("Type an action")
            .on_action(Message::UpdateActionText)
            .width(button_w),
        widget::Space::new().height(10),
        row![
            widget::text("Optional, additional instructions with GM powers:"),
            space::horizontal()
        ],
        widget::text_editor(gm_instruction_text_content)
            .placeholder("Type an action")
            .on_action(Message::UpdateGMInstructionText)
            .width(button_w),
        row![space::horizontal(), button("Go").on_press(Message::Submit)],
    ]
}
