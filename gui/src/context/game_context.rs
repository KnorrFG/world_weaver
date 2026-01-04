use std::mem;

use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use derive_more::{From, TryInto};
use iced::{Task, advanced::image::Handle as ImgHandle, widget::markdown};
use nonempty::nonempty;

use crate::{
    TryIntoExt,
    message::{ContextMessage, Message},
};
use engine::{
    game::{
        AdvanceResult, Game, Image, StartResultOrData, TurnData, TurnInput, TurnOutput,
        WorldDescription,
    },
    save_archive::SaveArchive,
};

pub struct GameContext {
    pub game: Game,
    pub save: SaveArchive,
    pub sub_state: SubState,
    pub output_markdown: Vec<markdown::Item>,
    pub output_text: String,
    pub image_data: Option<(ImgHandle, String)>,
}

#[derive(Debug, Default, Clone, From, TryInto)]
pub enum SubState {
    #[default]
    Uninit,
    Complete(Complete),
    WaitingForOutput(WaitingForOutput),
    WaitingForSummary(WaitingForSummary),
    InThePast(InThePast),
}

#[derive(Debug, Clone)]
pub struct Complete {
    pub turn_data: TurnData,
}

#[derive(Debug, Clone)]
pub struct WaitingForOutput {
    pub stream_buffer: String,
    pub input: TurnInput,
    pub output: Option<TurnOutput>,
    pub image: Option<Image>,
}

#[derive(Debug, Clone)]
pub struct WaitingForSummary {
    pub input: TurnInput,
    pub output: TurnOutput,
    pub image: Image,
}

#[derive(Debug, Clone)]
pub struct InThePast {
    pub completed_turn: usize,
    pub data: TurnData,
}

impl GameContext {
    pub fn try_new(game: Game, mut save: SaveArchive) -> Result<Self> {
        if let Some(td) = game.data.turn_data.last().cloned() {
            let output_markdown = markdown::parse(&td.output.text).collect();
            let image_data = Some((
                ImgHandle::from_bytes(save.read_image(*td.image_ids.last())?),
                td.image_captions.last().clone(),
            ));
            let output_text = td.output.text.clone();
            Ok(Self {
                game,
                save,
                sub_state: Complete { turn_data: td }.into(),
                output_markdown,
                image_data,
                output_text,
            })
        } else {
            Ok(Self {
                game,
                save,
                sub_state: SubState::Uninit,
                output_markdown: vec![],
                image_data: None,
                output_text: String::new(),
            })
        }
    }

    pub fn update(&mut self, message: ContextMessage) -> Result<Task<Message>> {
        use ContextMessage::*;
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
                    let output_fut = Task::perform(round_output, |res| OutputComplete(res).into());
                    let image_fut = Task::perform(image, |res| ImageReady(res).into());
                    let stream_task = Task::run(text_stream, |res| NewTextFragment(res).into());
                    self.sub_state = WaitingForOutput {
                        input,
                        output: None,
                        image: None,
                        stream_buffer: "".into(),
                    }
                    .into();
                    Ok(Task::batch([output_fut, stream_task, image_fut]))
                }
                StartResultOrData::Data(turn_data) => {
                    self.output_markdown = markdown::parse(&turn_data.output.text).collect();
                    self.image_data = Some((
                        ImgHandle::from_bytes(self.save.read_image(*turn_data.image_ids.first())?),
                        turn_data.image_captions.first().clone(),
                    ));
                    self.sub_state = Complete { turn_data }.into();
                    Ok(Task::none())
                }
            },

            OutputComplete(turn_output) => {
                let output = turn_output?;

                let WaitingForOutput {
                    stream_buffer,
                    input,
                    output: _,
                    image,
                } = self.sub_state.take().try_into_ex()?;

                if let Some(image) = image {
                    self.request_summary(input, output, image)
                } else {
                    self.sub_state = WaitingForOutput {
                        stream_buffer,
                        input,
                        output: Some(output),
                        image: None,
                    }
                    .into();
                    Ok(Task::none())
                }
            }

            SummaryFinished(message) => {
                let summary_msg = message?;
                let WaitingForSummary {
                    input,
                    output,
                    image,
                } = self.sub_state.take().try_into_ex()?;

                let id = self.save.append_image(&image.jpeg_bytes)?;
                self.game.update(
                    input,
                    output.clone(),
                    nonempty![id],
                    nonempty![image.caption],
                    summary_msg.map(|s| s.text),
                )?;
                self.save.write_game_data(&self.game.data)?;
                self.sub_state = Complete {
                    turn_data: self.game.data.turn_data.last().unwrap().clone(),
                }
                .into();
                Ok(Task::none())
            }

            NewTextFragment(t) => {
                let t = t?;
                self.sub_state.stream_buffer_mut()?.push_str(&t);
                self.output_text.push_str(&t);
                self.output_markdown = markdown::parse(&self.output_text).collect();
                Ok(Task::none())
            }

            ImageReady(image) => {
                let img = image?;
                let WaitingForOutput {
                    input,
                    output,
                    image: _,
                    stream_buffer,
                } = self.sub_state.take().try_into_ex()?;

                self.image_data = Some((
                    ImgHandle::from_bytes(img.jpeg_bytes.clone()),
                    img.caption.clone(),
                ));

                if let Some(output) = output {
                    self.request_summary(input, output, img)
                } else {
                    self.sub_state = WaitingForOutput {
                        input,
                        output: None,
                        image: Some(img),
                        stream_buffer,
                    }
                    .into();
                    Ok(Task::none())
                }
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
        self.image_data = Some((
            ImgHandle::from_bytes(self.save.read_image(*turn_data.image_ids.first())?),
            turn_data.image_captions.first().clone(),
        ));
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
                turn_data.output.secret_info = val.clone();
                self.game.data.turn_data.last_mut().unwrap().output.text = val.clone();
            }
            other => bail!("Invalid substate when seeing UpdateHiddenInfo: {other:#?}",),
        }

        self.output_text = val;
        self.output_markdown = markdown::parse(&self.output_text).collect();
        self.save.write_game_data(&self.game.data)?;
        Ok(())
    }

    fn request_summary(
        &mut self,
        input: TurnInput,
        output: TurnOutput,
        image: Image,
    ) -> Result<Task<Message>> {
        self.sub_state = WaitingForSummary {
            input,
            output,
            image,
        }
        .into();
        let fut = self.game.mk_summary_if_neccessary();
        Ok(Task::perform(fut, |res| {
            ContextMessage::SummaryFinished(res).into()
        }))
    }

    pub fn generate_new_turn(&mut self, input: TurnInput) -> Task<Message> {
        self.output_markdown.clear();
        self.output_text.clear();
        let AdvanceResult {
            text_stream,
            round_output,
            image,
        } = self.game.send_to_llm(input.clone());
        self.sub_state = WaitingForOutput {
            input,
            output: None,
            image: None,
            stream_buffer: "".into(),
        }
        .into();
        Task::batch([
            Task::perform(round_output, |x| ContextMessage::OutputComplete(x).into()),
            Task::perform(image, |x| ContextMessage::ImageReady(x).into()),
            Task::run(text_stream, |x| ContextMessage::NewTextFragment(x).into()),
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

    pub fn input(&self) -> Result<&TurnInput> {
        Ok(match &self.sub_state {
            SubState::InThePast(InThePast { data, .. }) => &data.input,
            SubState::Complete(Complete { turn_data }) => &turn_data.input,
            SubState::WaitingForOutput(WaitingForOutput { input, .. }) => input,
            SubState::WaitingForSummary(WaitingForSummary { input, .. }) => input,
            other => bail!("Invalid substate when getting input: {other:#?}",),
        })
    }

    pub fn regenerate_turn(&mut self, s: String) -> Result<Task<Message>> {
        let last_turn = self.sub_state.turn_data()?;
        let last_output = last_turn.output.text.clone();
        let last_input = last_turn.input.player_action.clone();
        self.load_prev_turn()?;
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
}

impl SubState {
    fn stream_buffer_mut(&mut self) -> Result<&mut String> {
        if let SubState::WaitingForOutput(WaitingForOutput { stream_buffer, .. }) = self {
            Ok(stream_buffer)
        } else {
            Err(eyre!("Can't provide stream_buffer while being: {self:#?}"))
        }
    }

    fn take(&mut self) -> Self {
        mem::take(self)
    }

    pub fn turn_data(&self) -> Result<&TurnData> {
        match self {
            Self::Complete(Complete { turn_data }) => Ok(turn_data),
            Self::InThePast(InThePast { data, .. }) => Ok(data),
            _ => Err(eyre!("Trying to get turn-data while being: {self:?}")),
        }
    }
}
