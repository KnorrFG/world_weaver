use std::{collections::BTreeMap, pin::Pin};

use crate::{
    HIST_SIZE, LLMBox, N_PROPOSED_OPTIONS,
    game::stream_finder::{MatchResult, StreamFinder},
    llm::{InputMessage, OutputMessage, Request, ResponseFragment},
};

use async_stream::try_stream;
use color_eyre::{
    Result,
    eyre::{bail, ensure},
};
use log::warn;
use serde::{Deserialize, Serialize};
use tokio::{pin, sync::oneshot};
use tokio_stream::{Stream, StreamExt};

mod stream_finder;

pub struct Game {
    llm: LLMBox,
    data: GameData,
}

impl Clone for Game {
    fn clone(&self) -> Self {
        Self {
            llm: self.llm.clone(),
            data: self.data.clone(),
        }
    }
}

impl Game {
    pub fn load(llm: LLMBox, data: GameData) -> Self {
        Game { llm, data }
    }

    pub fn try_new(
        llm: LLMBox,
        world_description: WorldDescription,
        player_character: String,
    ) -> Result<Self> {
        ensure!(
            world_description
                .pc_descriptions
                .contains_key(&player_character),
            "Invalid character name: {player_character}"
        );

        Ok(Game {
            llm,
            data: GameData {
                world_description,
                pc: player_character,
                summaries: vec![],
                turn_data: vec![],
            },
        })
    }

    pub fn send_to_llm(&self, input: TurnInput) -> AdvanceResult {
        let (tx, rx) = oneshot::channel();
        let req = self.data.construct_request(&input);
        let mut llm = self.llm.clone();
        let stream = try_stream! {
            let output = {
                let stream = llm.send_request_stream(req);
                let mut streaming = true;
                let mut finder = StreamFinder::new("<<<EOO>>>");

                pin!(stream);
                let output = loop {
                    if let Some(e) = stream.try_next().await? {
                        match e {
                            ResponseFragment::TextDelta(f) => {
                                if streaming {
                                    match finder.process(&f) {
                                        MatchResult::Blocked => {}
                                        MatchResult::CheckedOutput(output) => {
                                            yield output;
                                        }
                                        MatchResult::StopTokenMatched(processed) => {
                                            if !processed.is_empty() {
                                                yield processed;
                                            }

                                            streaming = false;
                                        }
                                    }
                                }
                            }
                            ResponseFragment::MessageComplete(m) => {
                                let output = TurnOutput::try_from(m)?;
                                break output;
                            }
                        }
                    }
                };

                // this will either error or return None
                stream.try_next().await?;
                output
            };
            _ = tx.send(output);

        };

        AdvanceResult {
            text_stream: Box::pin(stream),
            round_output: Box::pin(async move { Ok(rx.await?) }),
        }
    }

    pub fn get_data(&self) -> &GameData {
        &self.data
    }

    pub fn update(&mut self, input: TurnInput, output: TurnOutput) -> Result<()> {
        let turn_data = TurnData {
            summary_before_input: {
                let len = self.data.summaries.len();
                if len > 0 { Some(len - 1) } else { None }
            },
            input,
            output: output.clone(),
        };
        self.data.turn_data.push(turn_data);
        warn!("Summary not implemented");
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.data.turn_data.is_empty()
    }

    pub fn start_or_get_last_output<'a>(&'a mut self) -> StartResultOrOutput {
        if let Some(turn) = self.data.turn_data.last() {
            StartResultOrOutput::Output(turn.output.clone())
        } else {
            let input = TurnInput::PlayerAction(self.data.world_description.init_action.clone());
            StartResultOrOutput::StartResult(self.send_to_llm(input.clone()), input)
        }
    }
}

pub enum StartResultOrOutput {
    StartResult(AdvanceResult, TurnInput),
    Output(TurnOutput),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameData {
    pub world_description: WorldDescription,
    pub pc: String,
    pub summaries: Vec<Summary>,
    pub turn_data: Vec<TurnData>,
}

impl GameData {
    fn construct_request(&self, input: &TurnInput) -> Request {
        let player = &self.pc;
        let world_description = &self.world_description.main_description;
        let last_summary = self.summaries.last();
        let (summary, summary_turn) = match last_summary {
            Some(Summary { content, age }) => (content.as_str(), *age),
            None => ("", 0),
        };

        let system_message = indoc::formatdoc! {r#"
           You are a Story-teller-game. Below, I will provide a world-description.
           In that world, I control {player}. When I input anything, it's a command
           for {player} to execute in the world. Then it's your turn to decide and tell
           me how the world react to my input, and what happens. One pair of messages,
           one from me + one from you is called a turn.
           My input will be structured like this: The turn number, followed by
           three sections, all optional, like this:

           --- START EXAMPLE ---
            turn *N*
            # player action
            *whatever I want {player} to do or say.*
            # gm command
            *whatever I want you to respect while generating the next message.*
            # last secret info
            * The secret Info you generated for yourself last turn*
           --- END EXAMPLE ---

           The player action means: what ever I write here is what {player} does or says.
           When {player} is in a
           state that doesn't allow my inputted action, or makes it
           implausible, modify it by the least amount required to be possible,
           or interprete it in a way that makes it possible. These actions can fail.

           The gm command means: that I want control over the story, and you should
           respect it to the best of your abilities. 

           If I provide neither of those, it just means you should generate more output for the
           previous input.

           I will have you generate secret infos for each output, that I'll pass back
           to the input, which is the third section.

           The output should have the following structure:
           --- START EXAMPLE ---
           *The output*: text that is displayed to me, this should be between 300 and 2000 words

           <<<EOO>>>
           *Secret info*:. Stuff that is related to output, but hidden from me,
           it's a note for yourself. It should be between 100 and 1000 words.
           <<<EOS>>>
           Proposed Action 1
           <<<EOA>>>
           Proposed Action 2
           <<<EOA>>>
           Proposed Action 3
           --- END EXAMPLE ---

           The above example is explanatory, you are supposed to replace all text within it,
           except for <<<EOO>>>, <<<EOS>>> and <<<EOA>>>, which are parsing delimiters and
           need to appear exactly like this on their own lines. So your generated output
           should NOT start with `The output*:`

           The Proposed Actions should be one sentence each, describing 3 different
           plausable next actions for {player} to take.

           Here is the description of the world the story plays in, and some some
           instructions about the style:
           --- START DESCRIPTION ---
           {world_description} 
           --- END DESCRIPTION ---

           Here is a summary of everthing that has happened up till turn {summary_turn}:

           --- START SUMMARY ---
           {summary} 
           --- END SUMMARY ---
        "#};

        let messages = (0..self.turn_data.len())
            .rev()
            .take(HIST_SIZE)
            .rev()
            .flat_map(|i| {
                let mut user_message = format!("turn {i}");
                let TurnData { input, output, .. } = &self.turn_data[i];
                let last_secret_info = if i > 0 {
                    self.turn_data.get(i - 1).map(|td| &td.output.secret_info)
                } else {
                    None
                };

                input.write_to_user_msg_string(&mut user_message);
                if let Some(secret_info) = last_secret_info {
                    user_message.push_str("\n# last secret info\n");
                    user_message.push_str(secret_info);
                }

                [
                    InputMessage::user(user_message),
                    InputMessage::assistant(output.to_llm_format()),
                ]
            });

        let mut latest_message = String::new();
        input.write_to_user_msg_string(&mut latest_message);
        if let Some(last_turn) = self.turn_data.last() {
            latest_message.push_str("\n# last secret info\n");
            latest_message.push_str(&last_turn.output.secret_info);
        }

        let messages = messages
            .chain([InputMessage::user(latest_message)])
            .collect();
        Request {
            messages,
            max_tokens: 3000,
            system: Some(system_message),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub content: String,
    /// the turn after which it was created
    pub age: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnData {
    pub summary_before_input: Option<usize>,
    pub input: TurnInput,
    pub output: TurnOutput,
}

pub struct AdvanceResult {
    pub text_stream: Pin<Box<dyn Stream<Item = Result<String>> + Send>>,
    pub round_output: Pin<Box<dyn Future<Output = Result<TurnOutput>> + Send>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnOutput {
    pub text: String,
    pub secret_info: String,
    pub proposed_next_actions: [String; N_PROPOSED_OPTIONS],
    pub input_tokens: usize,
    pub output_tokens: usize,
}

impl TurnOutput {
    fn to_llm_format(&self) -> String {
        let mut output = String::new();

        output.push_str(&self.text);

        output.push_str("\n<<<EOO>>>\n");
        output.push_str(&self.secret_info);
        output.push_str("\n<<<EOS>>>\n");
        output.push_str(&self.proposed_next_actions.join("\n<<<EOA>>>\n"));

        output
    }
}

impl TryFrom<OutputMessage> for TurnOutput {
    type Error = color_eyre::Report;

    fn try_from(value: OutputMessage) -> std::result::Result<Self, Self::Error> {
        let parts = value.text.split("<<<EOO>>>").collect::<Vec<&str>>();
        let [output, tail] = parts[..] else {
            bail!("No <<<EOO>>> in output");
        };

        let parts = tail.split("<<<EOS>>>").collect::<Vec<&str>>();
        let [secret, tail] = parts[..] else {
            bail!("No in <<<EOS>>> in output");
        };

        let proposed_next_actions: Vec<String> = tail
            .split("<<<EOA>>>")
            .map(|s| s.trim().to_string())
            .collect();

        ensure!(
            proposed_next_actions.len() >= N_PROPOSED_OPTIONS,
            "Expected {} proposed actions, found {} Message: \n{}",
            N_PROPOSED_OPTIONS,
            proposed_next_actions.len(),
            value.text
        );

        Ok(TurnOutput {
            text: output.trim().to_string(),
            secret_info: secret.trim().to_string(),
            proposed_next_actions: proposed_next_actions[..N_PROPOSED_OPTIONS]
                .to_vec()
                .try_into()
                .unwrap(),
            input_tokens: value.input_tokens,
            output_tokens: value.output_tokens,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnInput {
    PlayerAction(String),
    GmInstruction(String),
    Both {
        player_action: String,
        gm_instruction: String,
    },
}

impl TurnInput {
    pub fn write_to_user_msg_string(&self, user_message: &mut String) {
        match self {
            TurnInput::PlayerAction(a) => {
                user_message.push_str("\n# player action\n");
                user_message.push_str(a);
            }
            TurnInput::GmInstruction(i) => {
                user_message.push_str("\n# gm command\n");
                user_message.push_str(i);
            }
            TurnInput::Both {
                player_action,
                gm_instruction,
            } => {
                user_message.push_str("\n# player action\n");
                user_message.push_str(player_action);
                user_message.push_str("\n# gm command\n");
                user_message.push_str(gm_instruction);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldDescription {
    pub main_description: String,
    pub pc_descriptions: BTreeMap<String, String>,
    pub init_action: String,
}
