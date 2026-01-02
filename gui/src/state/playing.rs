use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use engine::game::{TurnInput, TurnOutput};
use iced::{
    Color, Element, Length, Task, Theme,
    alignment::{Horizontal, Vertical},
    padding,
    widget::{
        self, Button, Column, Container, button, container, markdown, row, scrollable, space,
        text_editor::{self, Edit},
        text_input,
    },
};

use crate::{
    ElemHelper, State, TryIntoExt,
    context::game_context::{Complete, GameContext as Context, InThePast, SubState},
    elem_list, italic_text,
    message::{Message, UiMessage, ui_messages::Playing as MyMessage},
    state::{MainMenu, Modal, StateCommand, cmd, modal::confirm::ConfirmDialog},
};

#[derive(Debug, Clone)]
pub struct Playing {
    goto_turn_input: Option<usize>,
    action_text_content: text_editor::Content,
    gm_instruction_text_content: text_editor::Content,
}

enum EditorId {
    PlayerAction,
    GMInstruction,
}

impl Playing {
    pub fn new() -> Self {
        Self {
            goto_turn_input: None,
            action_text_content: text_editor::Content::default(),
            gm_instruction_text_content: text_editor::Content::default(),
        }
    }

    fn reset_action_editors(&mut self) {
        self.action_text_content = text_editor::Content::default();
        self.gm_instruction_text_content = text_editor::Content::default();
    }

    fn update_editor_content(
        &mut self,
        action: text_editor::Action,
        editor: EditorId,
    ) -> Result<StateCommand, color_eyre::eyre::Error> {
        if let text_editor::Action::Edit(Edit::Enter) = action {
            cmd::task(Task::done(MyMessage::Submit))
        } else {
            match editor {
                EditorId::PlayerAction => self.action_text_content.perform(action),
                EditorId::GMInstruction => self.gm_instruction_text_content.perform(action),
            }
            cmd::none()
        }
    }

    fn goto_turn_string(&self) -> String {
        self.goto_turn_input
            .as_ref()
            .map(|x| x.to_string())
            .unwrap_or("".into())
    }
}

impl State for Playing {
    fn update(
        &mut self,
        message: UiMessage,
        ctx: &mut crate::context::Context,
    ) -> color_eyre::eyre::Result<StateCommand> {
        let ctx = ctx
            .game
            .as_mut()
            .ok_or(eyre!("No game in context while being in playing state"))?;

        use MyMessage::*;
        match message.try_into_ex()? {
            UpdateActionText(action) => self.update_editor_content(action, EditorId::PlayerAction),
            UpdateGMInstructionText(action) => {
                self.update_editor_content(action, EditorId::GMInstruction)
            }
            ProposedActionButtonPressed(s) => {
                if self.action_text_content.text() == s {
                    cmd::task(Task::done(Submit))
                } else {
                    self.action_text_content = text_editor::Content::with_text(&s);
                    cmd::none()
                }
            }
            Submit => {
                let input = TurnInput {
                    player_action: self.action_text_content.text(),
                    gm_instruction: self.gm_instruction_text_content.text(),
                };
                self.reset_action_editors();
                cmd::task(ctx.generate_new_turn(input))
            }
            PrevTurnButtonPressed => {
                ctx.load_prev_turn()?;
                cmd::none()
            }
            NextTurnButtonPressed => {
                ctx.load_next_turn()?;
                cmd::none()
            }
            UpdateTurnInput(inp) => {
                self.goto_turn_input = inp.parse().ok();
                cmd::none()
            }
            GotoTurnPressed => {
                if let Some(target) = self.goto_turn_input {
                    ensure!(
                        (1..=ctx.game.current_turn()).contains(&target),
                        "Invalid turn number"
                    );
                    ctx.load_completed_turn(target - 1)?;
                }
                cmd::none()
            }
            GoToCurrentTurn => {
                ctx.load_completed_turn(ctx.game.current_turn() - 1)?;
                cmd::none()
            }
            LoadGameFromCurrentPastButtonPressed => cmd::transition(Modal::new(
                State::clone(self),
                ConfirmDialog::new(
                    "Do you really want to load the Game from here?\nThis will delete all unsafed progress.",
                    Some(ConfirmLoadGameFromCurrentPast.into()),
                    None,
                ),
            )),
            ConfirmLoadGameFromCurrentPast => {
                ctx.load_from_current_past()?;
                self.reset_action_editors();
                cmd::none()
            }
            ShowHiddenText => {
                let hidden_info = ctx.hidden_info()?;
                cmd::transition(Modal::edit(
                    State::clone(self),
                    "Hidden Information",
                    hidden_info,
                    |msg| Task::done(UpdateHiddenInfo(msg).into()),
                ))
            }
            UpdateHiddenInfo(val) => {
                ctx.update_hidden_info(val)?;
                cmd::none()
            }
            ShowImageDescription => {
                let img_info = ctx.image_info()?;
                cmd::transition(Modal::message(
                    State::clone(self),
                    "Image Description",
                    img_info,
                ))
            }
            CopyInputToClipboard => {
                let input = ctx.input()?;
                cmd::task(iced::clipboard::write::<Message>(
                    input.player_action.clone(),
                ))
            }
            RegenerateButtonPressed => cmd::transition(Modal::edit(
                State::clone(self),
                "What do you want to change",
                "",
                |s| Task::done(MyMessage::RegenerateMessage(s).into()),
            )),
            RegenerateMessage(s) => {
                self.reset_action_editors();
                cmd::task(ctx.regenerate_turn(s)?)
            }
            ToMainMenu => cmd::transition(MainMenu::try_new()?),
        }
    }

    fn view<'a>(&'a self, ctx: &'a crate::context::Context) -> iced::Element<'a, UiMessage> {
        let ctx = ctx
            .game
            .as_ref()
            .expect("No game in context while being in playing state");

        let mut sidebar = Column::new();
        if let Some((handle, caption)) = &ctx.image_data {
            sidebar = sidebar.extend([
                container(widget::image(handle).height(Length::Fill).expand(true))
                    .max_width(832)
                    .into(),
                if ctx.sub_state.turn_data().is_ok() {
                    row![
                        widget::text(caption),
                        widget::button("👁").on_press(MyMessage::ShowImageDescription.into())
                    ]
                    .align_y(Vertical::Center)
                    .spacing(10)
                    .into_elem()
                } else {
                    widget::text(caption).into_elem()
                },
            ]);
            // .width(Length::Shrink);
        };

        let mut main_col: Vec<Element<UiMessage>> = vec![];
        let mut text_col: Vec<Element<UiMessage>> = vec![];
        if let Ok(ti) = ctx.input() {
            text_col.push(italic_text(&ti.player_action).into());
            text_col.push(
                widget::row![
                    space::horizontal(),
                    widget::button("📋").on_press(MyMessage::CopyInputToClipboard.into())
                ]
                .into(),
            );
            text_col.push(widget::rule::horizontal(2).into());
        }

        text_col.push(
            markdown::view(&ctx.output_markdown, Theme::TokyoNight)
                .map(|_| unreachable!())
                .into(),
        );

        main_col.push(widget::column(text_col).spacing(20).into());

        let button_w = 500;
        match &ctx.sub_state {
            SubState::Complete(Complete { turn_data }) => {
                let elems = mk_input_ui_portion(
                    &turn_data.output,
                    button_w,
                    &self.action_text_content,
                    &self.gm_instruction_text_content,
                )
                .into_iter()
                .chain(elem_list![
                    widget::rule::horizontal(1),
                    mk_turn_selection_buttons(
                        ctx,
                        ctx.game.current_turn(),
                        &self.goto_turn_string(),
                    ),
                    row![
                        space::horizontal(),
                        button("change turn").on_press(MyMessage::RegenerateButtonPressed.into()),
                        space::horizontal(),
                    ]
                ]);
                main_col.extend([
                    mk_view_hidden_info_button().into(),
                    widget::column(elems)
                        .max_width(500)
                        .spacing(15)
                        .align_x(Horizontal::Center)
                        .into(),
                ]);
            }
            SubState::InThePast(InThePast {
                completed_turn: turn,
                data: _data,
            }) => {
                let elems = elem_list![
                    widget::Space::new().height(20),
                    mk_turn_selection_buttons(ctx, *turn, &self.goto_turn_string()),
                    button("Goto current turn").on_press(MyMessage::GoToCurrentTurn.into()),
                    button("Load game from here")
                        .on_press(MyMessage::LoadGameFromCurrentPastButtonPressed.into())
                ];
                main_col.extend(elem_list![
                    mk_view_hidden_info_button(),
                    widget::column(elems)
                        .max_width(500)
                        .spacing(15)
                        .align_x(Horizontal::Center)
                ]);
            }
            _ => {}
        }

        let text_row = row![
            container(scrollable(
                container(widget::column(main_col).align_x(Horizontal::Center))
                    .padding(padding::all(10.).right(20.))
            ))
            .width(700)
            .padding(10)
            .style(|_theme| container::background(Color::from_rgb(0.95, 0.95, 0.95))),
            sidebar.align_x(Horizontal::Center).height(Length::Fill)
        ]
        .spacing(20);

        let main_col = widget::column![
            mk_header(ctx),
            widget::rule::horizontal(2),
            container(text_row).center_x(Length::Fill).padding(20)
        ]
        .align_x(Horizontal::Center)
        .max_width(1500)
        .spacing(10);

        container(main_col)
            .center_x(Length::Fill)
            .padding(padding::top(20))
            .into_elem()
        // .explain(iced::Color::from_rgb(1., 0., 0.))
    }

    fn clone(&self) -> Box<dyn State> {
        Box::new(Clone::clone(self))
    }
}

fn mk_header<'a>(ctx: &'a Context) -> Container<'a, UiMessage> {
    container(
        widget::row![
            widget::row![
                button("☰").on_press(MyMessage::ToMainMenu.into()),
                widget::space::horizontal()
            ]
            .align_y(Vertical::Center)
            .width(Length::FillPortion(1)),
            widget::text!("{} - Turn {}", ctx.game.world_name(), ctx.current_turn()).size(32),
            widget::Space::new().width(Length::FillPortion(1))
        ]
        .align_y(Vertical::Center),
    )
    .padding(10)
}
fn proposed_action_button<'a>(text: &'a str) -> Button<'a, UiMessage> {
    button(text).on_press(MyMessage::ProposedActionButtonPressed(text.into()).into())
}

fn mk_turn_selection_buttons<'a>(
    ctx: &'a Context,
    current_turn: usize,
    goto_turn_input: &str,
) -> row::Row<'a, UiMessage> {
    let mut row = vec![];
    if current_turn > 0 {
        row.push(
            widget::button("←")
                .on_press(MyMessage::PrevTurnButtonPressed.into())
                .into_elem(),
        );
    }

    row.extend(elem_list![
        widget::space::horizontal(),
        text_input("turn", goto_turn_input)
            .on_input(|t| MyMessage::UpdateTurnInput(t).into())
            .on_submit(MyMessage::GotoTurnPressed.into()),
        widget::button("Goto Turn").on_press(MyMessage::GotoTurnPressed.into()),
        widget::space::horizontal()
    ]);
    if current_turn < ctx.game.current_turn() - 1 {
        row.push(
            widget::button("→")
                .on_press(MyMessage::NextTurnButtonPressed.into())
                .into(),
        );
    }

    widget::row(row)
}

fn mk_input_ui_portion<'a>(
    output: &'a TurnOutput,
    button_w: u32,
    action_text_content: &'a text_editor::Content,
    gm_instruction_text_content: &'a text_editor::Content,
) -> impl IntoIterator<Item = Element<'a, UiMessage>> {
    elem_list![
        widget::Space::new().height(20),
        proposed_action_button(&output.proposed_next_actions[0]).width(button_w),
        proposed_action_button(&output.proposed_next_actions[1]).width(button_w),
        proposed_action_button(&output.proposed_next_actions[2]).width(button_w),
        widget::Space::new().height(10),
        row![widget::text("What to do next:"), space::horizontal()],
        widget::text_editor(action_text_content)
            .placeholder("Type an action")
            .on_action(|a| MyMessage::UpdateActionText(a).into())
            .width(button_w),
        widget::Space::new().height(10),
        row![
            widget::text("Optional, additional instructions with GM powers:"),
            space::horizontal()
        ],
        widget::text_editor(gm_instruction_text_content)
            .placeholder("Type an action")
            .on_action(|a| MyMessage::UpdateGMInstructionText(a).into())
            .width(button_w),
        row![
            space::horizontal(),
            button("Go").on_press(MyMessage::Submit.into())
        ],
    ]
}

fn mk_view_hidden_info_button() -> Column<'static, UiMessage> {
    widget::column![button("👁").on_press(MyMessage::ShowHiddenText.into())]
        .width(Length::Fill)
        .align_x(Horizontal::Right)
}
