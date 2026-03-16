use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use iced::{Task, advanced::image::Handle as ImgHandle, widget::markdown};
use log::warn;

use crate::{
    TryIntoExt,
    message::{ContextMessage, Message, ui_messages::Playing as PlayingMessage},
};
use engine::{
    game::{
        AdvanceResult, Game, StartResultOrData, StoredImageInfo, TurnInput, WorldDescription,
    },
    save_archive::SaveArchive,
};

mod pending_turn;
mod state;

use pending_turn::{FinalizingTurn, PendingTurn, Resolution};
pub use pending_turn::ImageState;
pub use state::{Complete, InThePast, SubState};

pub struct GameContext {
    pub game: Game,
    pub save: SaveArchive,
    pub sub_state: SubState,
    pub current_generation: usize,
    pub output_scroll_y: f32,
    pub output_markdown: Vec<markdown::Item>,
    pub output_text: String,
    pub image_data: Option<ImageData>,
}

pub struct ImageData {
    pub handle: ImgHandle,
    pub caption: String,
    // if this is false, it implies the generation for the current image failed,
    // and this is an older one
    pub is_current: bool,
}

impl GameContext {
    pub fn try_new(game: Game, mut save: SaveArchive) -> Result<Self> {
        if let Some(td) = game.data.turn_data.last().cloned() {
            let output_markdown = markdown::parse(&td.output.text).collect();
            let image_data = game
                .get_latest_image_info()
                .map(|info| {
                    color_eyre::eyre::Ok(ImageData {
                        handle: ImgHandle::from_bytes(save.read_image(info.id)?),
                        caption: info.caption.clone(),
                        is_current: true,
                    })
                })
                .transpose()?;
            let output_text = td.output.text.clone();
            Ok(Self {
                game,
                save,
                sub_state: Complete { turn_data: td }.into(),
                output_markdown,
                image_data,
                output_text,
                current_generation: 0,
                output_scroll_y: 0.0,
            })
        } else {
            Ok(Self {
                game,
                save,
                sub_state: SubState::Uninit,
                output_markdown: vec![],
                image_data: None,
                output_text: String::new(),
                current_generation: 0,
                output_scroll_y: 0.0,
            })
        }
    }

    pub fn update(&mut self, message: ContextMessage) -> Result<Task<Message>> {
        use ContextMessage::*;

        /// this macro will make sure, if there was an error, that the sub-state
        /// is reset, and the generation is increased, so other incoming messages
        /// for the errornous turn will be ignored
        macro_rules! unpack_received_msg {
            ($invar:ident, $generation:ident) => {{
                if $generation < self.current_generation {
                    return Ok(Task::none());
                }

                let Ok(output) = $invar else {
                    self.current_generation += 1;
                    let turn = self.current_turn();
                    if turn > 0 {
                        self.load_completed_turn(self.current_turn() - 1)?;
                    }
                    bail!(indoc::formatdoc! {"
                        There was an error with the LLM response or the image model.
                        This can happen. Try again.
                        If it repeats, try doing something else, if you're on Flux2, try Flux1.
                        Details:
                        {:?}", $invar});
                };
                output
            }}
        }

        match message {
            Init => match self.game.start_or_get_last_output() {
                StartResultOrData::StartResult(
                    AdvanceResult {
                        text_stream,
                        round_output,
                        image,
                    },
                    input,
                ) => {
                    let generation = self.current_generation;
                    let output_fut = Task::perform(round_output, move |res| {
                        OutputComplete(generation, res).into()
                    });
                    let image_fut =
                        Task::perform(image, move |res| ImageReady(generation, res).into());
                    let stream_task = Task::run(text_stream, move |res| {
                        NewTextFragment(generation, res).into()
                    });
                    self.sub_state = PendingTurn::new(input).into();
                    Ok(Task::batch([output_fut, stream_task, image_fut]))
                }
                StartResultOrData::Data(turn_data) => {
                    self.output_markdown = markdown::parse(&turn_data.output.text).collect();
                    self.image_data = turn_data
                        .images
                        .first()
                        .map(|info| {
                            color_eyre::eyre::Ok(ImageData {
                                handle: ImgHandle::from_bytes(self.save.read_image(info.id)?),
                                caption: info.caption.clone(),
                                is_current: true,
                            })
                        })
                        .transpose()?;

                    self.sub_state = Complete { turn_data }.into();
                    Ok(Task::none())
                }
            },

            OutputComplete(generation, turn_output) => {
                let output = unpack_received_msg!(turn_output, generation);

                self.output_text = output.text.clone();
                self.output_markdown = markdown::parse(&self.output_text).collect();

                let pending_turn: PendingTurn = self.sub_state.take().try_into_ex()?;
                self.apply_resolution(pending_turn.finish_output(output))
            }

            SummaryFinished(generation, message) => {
                let summary_msg = unpack_received_msg!(message, generation);
                let FinalizingTurn {
                    input,
                    output,
                    image,
                } = self.sub_state.take().try_into_ex()?;

                let images = if let Some(image) = image {
                    let id = self.save.append_image(&image.jpeg_bytes)?;
                    vec![StoredImageInfo {
                        id,
                        caption: image.caption,
                    }]
                } else {
                    vec![]
                };
                self.game.update(
                    input,
                    output.clone(),
                    images,
                    summary_msg.map(|s| s.text),
                )?;
                self.save.write_game_data(&self.game.data)?;
                self.sub_state = Complete {
                    turn_data: self.game.data.turn_data.last().unwrap().clone(),
                }
                .into();
                self.current_generation += 1;
                Ok(Task::done(PlayingMessage::ClearActionEditors.into()))
            }

            NewTextFragment(generation, t) => {
                let t = unpack_received_msg!(t, generation);
                self.sub_state.stream_buffer_mut()?.push_str(&t);
                self.output_text.push_str(&t);
                self.output_markdown = markdown::parse(&self.output_text).collect();
                Ok(Task::none())
            }

            ImageReady(generation, image) => {
                if generation < self.current_generation {
                    return Ok(Task::none());
                }
                let Ok(img) = image else {
                    if let Some(img_data) = &mut self.image_data {
                        img_data.is_current = false;
                    }
                    warn!(
                        "{}",
                        indoc::formatdoc! {
                         "
                            There was an error with the image model.
                            This can happen. Try again. If you're on Flux2, try Flux1.
                            Details:
                            {:?}",image
                        }
                    );

                    let pending_turn: PendingTurn = self.sub_state.take().try_into_ex()?;
                    return self.apply_resolution(pending_turn.fail_image());
                };
                let pending_turn: PendingTurn = self.sub_state.take().try_into_ex()?;

                self.image_data = Some(ImageData {
                    handle: ImgHandle::from_bytes(img.jpeg_bytes.clone()),
                    caption: img.caption.clone(),
                    is_current: true,
                });

                self.apply_resolution(pending_turn.finish_image(img))
            }
        }
    }

    /// turn semantics are as follows:
    /// when the game starts, that's turn 0, before there is any input or output
    /// the result of the 0th turn is stored in game.data_turn_data[0].
    /// As soon as you finish the first turn (index 0), you are in turn 1.
    /// But in turn 1, you do see the outputs of turn 0;
    pub fn current_turn(&self) -> usize {
        match &self.sub_state {
            SubState::InThePast(InThePast {
                completed_turn,
                data: _data,
            }) => *completed_turn + 1,
            _ => self.game.current_turn(),
        }
    }

    /// loading completed turn n actually means loading turn n+1, but this way it's less confusing
    pub fn load_completed_turn(&mut self, target_turn: usize) -> Result<()> {
        let turn_data = self
            .game
            .data
            .turn_data
            .get(target_turn)
            .ok_or(eyre!("Invalid target turn: {target_turn}"))?;
        self.image_data = self
            .game
            .get_latest_image_info_for_turn(target_turn)
            .map(|info| {
                color_eyre::eyre::Ok(ImageData {
                    handle: ImgHandle::from_bytes(self.save.read_image(info.id)?),
                    caption: info.caption.clone(),
                    is_current: turn_data.images.first().map(|i| i.id) == Some(info.id),
                })
            })
            .transpose()?;
        self.output_markdown = markdown::parse(&turn_data.output.text).collect();

        // this looks wrong but is right. If we load the completed turn 0, the displayed output
        // is the ouput of turn 0, but that means we're actually in turn 1
        if target_turn + 1 == self.game.current_turn() {
            self.sub_state = Complete {
                turn_data: turn_data.clone(),
            }
            .into();
        } else {
            self.sub_state = InThePast {
                completed_turn: target_turn,
                data: turn_data.clone(),
            }
            .into();
        }
        Ok(())
    }

    pub fn update_hidden_info(&mut self, val: String) -> Result<()> {
        match &mut self.sub_state {
            SubState::InThePast(InThePast {
                data,
                completed_turn,
            }) => {
                data.output.secret_info = val.clone();
                self.game.data.turn_data[*completed_turn].output.secret_info = val;
            }
            SubState::Complete(Complete { turn_data }) => {
                turn_data.output.secret_info = val.clone();
                self.game
                    .data
                    .turn_data
                    .last_mut()
                    .unwrap()
                    .output
                    .secret_info = val;
            }
            other => bail!("Invalid substate when seeing UpdateHiddenInfo: {other:#?}",),
        }

        self.save.write_game_data(&self.game.data)?;
        Ok(())
    }

    pub fn update_output(&mut self, val: String) -> Result<()> {
        match &mut self.sub_state {
            SubState::InThePast(InThePast {
                data,
                completed_turn,
            }) => {
                data.output.text = val.clone();
                self.game.data.turn_data[*completed_turn].output.text = val.clone();
            }
            SubState::Complete(Complete { turn_data }) => {
                turn_data.output.text = val.clone();
                self.game.data.turn_data.last_mut().unwrap().output.text = val.clone();
            }
            other => bail!("Invalid substate when seeing UpdateHiddenInfo: {other:#?}",),
        }

        self.output_text = val;
        self.output_markdown = markdown::parse(&self.output_text).collect();
        self.save.write_game_data(&self.game.data)?;
        Ok(())
    }

    fn request_summary(&mut self, turn: FinalizingTurn) -> Result<Task<Message>> {
        self.sub_state = turn.into();
        let fut = self.game.mk_summary_if_neccessary();
        let generation = self.current_generation;
        Ok(Task::perform(fut, move |res| {
            ContextMessage::SummaryFinished(generation, res).into()
        }))
    }

    fn apply_resolution(&mut self, resolution: Resolution) -> Result<Task<Message>> {
        match resolution {
            Resolution::Pending(turn) => {
                self.sub_state = turn.into();
                Ok(Task::none())
            }
            Resolution::Finalizing(turn) => self.request_summary(turn),
        }
    }

    pub fn generate_new_turn(&mut self, input: TurnInput) -> Task<Message> {
        self.output_markdown.clear();
        self.output_text.clear();
        let AdvanceResult {
            text_stream,
            round_output,
            image,
        } = self.game.send_to_llm(input.clone());
        self.sub_state = PendingTurn::new(input).into();
        let generation = self.current_generation;
        Task::batch([
            Task::perform(round_output, move |x| {
                ContextMessage::OutputComplete(generation, x).into()
            }),
            Task::perform(image, move |x| {
                ContextMessage::ImageReady(generation, x).into()
            }),
            Task::run(text_stream, move |x| {
                ContextMessage::NewTextFragment(generation, x).into()
            }),
        ])
    }

    pub fn load_prev_turn(&mut self) -> Result<()> {
        let target_turn = match &self.sub_state {
            SubState::Complete(_) => self.game.current_turn() - 2,
            SubState::InThePast(InThePast {
                completed_turn: turn,
                ..
            }) => *turn - 1,
            other => bail!(
                "PrevTurnButtonPressed but Substate is not Complete or InThePast: {:?}",
                other
            ),
        };
        self.load_completed_turn(target_turn)
    }

    pub fn load_next_turn(&mut self) -> Result<()> {
        let target_turn = match &self.sub_state {
            SubState::InThePast(InThePast {
                completed_turn: turn,
                ..
            }) => *turn + 1,
            other => bail!(
                "PrevTurnButtonPressed but Substate is not Complete or InThePast: {:?}",
                other
            ),
        };
        self.load_completed_turn(target_turn)
    }

    pub fn load_from_current_past(&mut self) -> Result<()> {
        let InThePast {
            completed_turn,
            data,
        } = self.sub_state.take().try_into_ex()?;

        self.save.clip_after_turn(completed_turn)?;
        self.game.data = self.save.read_game_data()?;
        self.sub_state = Complete { turn_data: data }.into();
        Ok(())
    }

    pub fn hidden_info(&self) -> Result<&str> {
        Ok(match &self.sub_state {
            SubState::InThePast(InThePast { data, .. }) => &data.output.secret_info,
            SubState::Complete(Complete { turn_data }) => &turn_data.output.secret_info,
            other => bail!("Invalid substate when getting hidden info: {other:#?}",),
        })
    }

    pub fn image_info(&self) -> Result<&str> {
        Ok(match &self.sub_state {
            SubState::InThePast(InThePast { data, .. }) => &data.output.image_description,
            SubState::Complete(Complete { turn_data }) => &turn_data.output.image_description,
            other => bail!("Invalid substate when getting image info: {other:#?}",),
        })
    }

    fn summary_idx_for_current_turn(&self) -> Result<Option<usize>> {
        let turn = self.current_turn();
        if turn < self.game.current_turn() {
            Ok(self
                .game
                .data
                .turn_data
                .get(turn)
                .ok_or(eyre!("Invalid turn index: {turn}"))?
                .summary_before_input)
        } else {
            Ok(self.game.data.summaries.len().checked_sub(1))
        }
    }

    pub fn summary_for_current_turn(&self) -> Result<Option<String>> {
        let summary_idx = self.summary_idx_for_current_turn()?;
        Ok(summary_idx.and_then(|i| self.game.data.summaries.get(i).map(|s| s.content.clone())))
    }

    pub fn update_summary_for_current_turn(&mut self, val: String) -> Result<()> {
        let summary_idx = self
            .summary_idx_for_current_turn()?
            .ok_or(eyre!("No summary is available for this turn"))?;
        self.game
            .data
            .summaries
            .get_mut(summary_idx)
            .ok_or(eyre!("Invalid summary index: {summary_idx}"))?
            .content = val;
        self.save.write_game_data(&self.game.data)?;
        Ok(())
    }

    pub fn input(&self) -> Result<&TurnInput> {
        Ok(match &self.sub_state {
            SubState::InThePast(InThePast { data, .. }) => &data.input,
            SubState::Complete(Complete { turn_data }) => &turn_data.input,
            SubState::WaitingForOutput(PendingTurn { input, .. }) => input,
            SubState::WaitingForSummary(FinalizingTurn { input, .. }) => input,
            other => bail!("Invalid substate when getting input: {other:#?}",),
        })
    }

    pub fn regenerate_turn(&mut self, s: String) -> Result<Task<Message>> {
        let last_turn = self.sub_state.turn_data()?;
        let last_output = last_turn.output.text.clone();
        let last_input = last_turn.input.player_action.clone();
        self.load_prev_turn()?;
        self.load_from_current_past()?;
        Ok(self.generate_new_turn(TurnInput {
            player_action: last_input,
            gm_instruction: indoc::formatdoc!(
                "
                        This was your last attempts to generate the next turn:
        
                        ---------START-------
                        {last_output}
                        --------END---------
        
                        Use that as base for what should happen, but modify it like this:
                        {s}"
            ),
        }))
    }

    pub(crate) fn upate_world_description(&mut self, world: WorldDescription) -> Result<()> {
        self.game.data.world_description = world;
        self.save.write_game_data(&self.game.data)?;
        Ok(())
    }

    pub fn set_output_scroll_y(&mut self, y: f32) {
        self.output_scroll_y = y.clamp(0.0, 1.0);
    }
}
