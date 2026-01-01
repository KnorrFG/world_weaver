use std::{collections::BTreeMap, pin::Pin};

use crate::{
    HIST_SIZE, ImgModBox, LLMBox, N_PROPOSED_OPTIONS,
    game::stream_finder::{MatchResult, StreamFinder},
    image_model::{self},
    llm::{InputMessage, OutputMessage, Request, ResponseFragment},
};

use async_stream::try_stream;
use color_eyre::{
    Result,
    eyre::{bail, ensure},
};
use log::debug;
use nonempty::NonEmpty;
use serde::{Deserialize, Serialize};
use tokio::{pin, sync::oneshot};
use tokio_stream::{Stream, StreamExt};

mod stream_finder;

const SUMMARY_INTERVAL: usize = 8;

pub struct Game {
    pub llm: LLMBox,
    pub imgmod: ImgModBox,
    pub data: GameData,
}

impl Clone for Game {
    fn clone(&self) -> Self {
        Self {
            llm: self.llm.clone(),
            data: self.data.clone(),
            imgmod: self.imgmod.clone(),
        }
    }
}

enum SendToLLMState {
    ParsingImageDescription,
    StreamingOutputText,
    FinishingUp,
}

pub struct AdvanceResult {
    pub image: Pin<Box<dyn Future<Output = Result<Image>> + Send>>,
    pub text_stream: Pin<Box<dyn Stream<Item = Result<String>> + Send>>,
    pub round_output: Pin<Box<dyn Future<Output = Result<TurnOutput>> + Send>>,
}

impl Game {
    pub fn load(llm: LLMBox, imgmod: ImgModBox, data: GameData) -> Self {
        Game { llm, data, imgmod }
    }

    pub fn try_new(
        llm: LLMBox,
        imgmod: ImgModBox,
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
            imgmod,
            data: GameData {
                world_description,
                pc: player_character,
                summaries: vec![],
                turn_data: vec![],
            },
        })
    }

    pub fn send_to_llm(&self, input: TurnInput, extra_img_infos: &str) -> AdvanceResult {
        let (tx_output, rx_output) = oneshot::channel();
        let (tx_img_description, rx_img_description) = oneshot::channel();
        let mut tx_img_description = Some(tx_img_description);
        let req = self.data.construct_request(&input, extra_img_infos);
        let mut llm = self.llm.clone();

        let mut mode = SendToLLMState::ParsingImageDescription;

        let stream = try_stream! {
            let output = {
                let stream = llm.send_request_stream(req);
                let mut eoo_finder = StreamFinder::new("<<<EOO>>>");
                let mut eoi_finder = StreamFinder::new("<<<EOIC>>>");
                let mut image_description = String::new();
                let mut post_eoi_text = None;

                pin!(stream);
                let output = loop {
                    if let Some(e) = stream.try_next().await? {
                        match e {
                            ResponseFragment::TextDelta(f) => {
                                match mode {
                                    SendToLLMState::ParsingImageDescription => {
                                        match eoi_finder.process(&f) {
                                            MatchResult::Blocked => {},
                                            MatchResult::CheckedOutput(o) => {
                                                image_description.push_str(&o);
                                            },
                                            MatchResult::StopTokenMatched {
                                                pre_token_text,
                                                post_token_text } => {
                                                image_description.push_str(&pre_token_text);
                                                post_eoi_text = Some(post_token_text);
                                                mode = SendToLLMState::StreamingOutputText;

                                                let description = parse_image_description(&image_description)?;
                                                _ = tx_img_description.take().expect("finished parsing image description a second time. This should be impossible. It's a bug").send(description);
                                            },
                                        }
                                    },
                                    SendToLLMState::StreamingOutputText => {
                                        match eoo_finder.process(&f) {
                                            MatchResult::Blocked => {}
                                            MatchResult::CheckedOutput(output) => {
                                                if let Some(mut prefix) = post_eoi_text.take() {
                                                    prefix.push_str(&output);
                                                    yield prefix;

                                                } else {
                                                    yield output;
                                                }
                                            }
                                            MatchResult::StopTokenMatched{
                                                pre_token_text: processed,
                                                post_token_text: _,
                                            } => {
                                                if !processed.is_empty() {
                                                    yield processed;
                                                }

                                                mode = SendToLLMState::FinishingUp
                                            }
                                        }
                                    },
                                    SendToLLMState::FinishingUp => {},
                                }
                            }
                            ResponseFragment::MessageComplete(m) => {
                                debug!("Output complete:\n{}", m.text);
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
            _ = tx_output.send(output);

        };

        AdvanceResult {
            image: Box::pin(get_image(rx_img_description, self.imgmod.clone())),
            text_stream: Box::pin(stream),
            round_output: Box::pin(async move { Ok(rx_output.await?) }),
        }
    }

    pub fn current_turn(&self) -> usize {
        self.data.turn_data.len()
    }

    pub fn world_name(&self) -> &str {
        &self.data.world_description.name
    }

    pub fn mk_summary_if_neccessary(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<OutputMessage>>> + Send + 'static>> {
        let current_turn = self.data.turn_data.len() as isize - 1;
        let should_summarize = match self.data.summaries.last() {
            Some(s) => current_turn - s.bday as isize >= SUMMARY_INTERVAL as isize,
            None => current_turn >= SUMMARY_INTERVAL as isize,
        };
        let tdl = self.data.turn_data.len();

        if should_summarize {
            debug!("updating summary");
            let llm = self.llm.clone();
            let last_summary = self
                .data
                .summaries
                .last()
                .map(|s| s.content.as_str())
                .unwrap_or("")
                .to_string();
            let turns = self.data.turn_data[tdl - SUMMARY_INTERVAL..tdl].to_vec();
            Box::pin(async move {
                let summary = create_new_summary(llm, &last_summary, turns).await?;
                debug!("New summary:\n{}", summary.text);
                Ok(Some(summary))
            })
        } else {
            Box::pin(async { Ok(None) })
        }
    }

    pub fn update(
        &mut self,
        input: TurnInput,
        output: TurnOutput,
        image_ids: NonEmpty<usize>,
        image_captions: NonEmpty<String>,
        summary: Option<String>,
    ) -> Result<()> {
        let turn_data = TurnData {
            summary_before_input: {
                let len = self.data.summaries.len();
                if len > 0 { Some(len - 1) } else { None }
            },
            input,
            output: output.clone(),
            image_ids,
            image_captions,
        };
        self.data.turn_data.push(turn_data);

        if let Some(content) = summary {
            self.data.summaries.push(Summary {
                content,
                bday: self.data.turn_data.len() - 1,
            });
        }

        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.data.turn_data.is_empty()
    }

    pub fn start_or_get_last_output<'a>(&'a mut self) -> StartResultOrData {
        if let Some(turn) = self.data.turn_data.last() {
            StartResultOrData::Data(turn.clone())
        } else {
            let input = TurnInput::player_action(self.data.world_description.init_action.clone());
            StartResultOrData::StartResult(
                self.send_to_llm(
                    input.clone(),
                    self.imgmod.model().extra_generation_instructions(),
                ),
                input,
            )
        }
    }
}

async fn create_new_summary(
    mut llm: LLMBox,
    last_summary: &str,
    turns: Vec<TurnData>,
) -> Result<OutputMessage> {
    let system_message = indoc::indoc! {r#"
            You are a summarization component for an ongoing narrative game.

            This game normally runs in a storyteller mode with strict input and output formats.
            You must UNDERSTAND those formats, but you must NOT produce output in those formats.

            NORMAL GAME CONTEXT (for understanding only):

            - The game progresses in discrete TURNS.
            - Each turn consists of:
              1) An input, consisting of the turn number and three optional components:
                  - a player instruction that is a command for the player character to execute.
                    these commands can fail, if appropriate in the world
                  - a gm instruction, which should be respected by the story teller, and which
                    might contain important information that should end up in the summary.
                  - hidden information about the last output that weren't displayed to the player,
                    but may not the less contain important information.
              2) A storyteller (assistant) response shown to the player

            INPUT FORMAT FOR THIS TASK:

            You will receive:
            - The previous summary (if it exists)
            - A sequence of rounds since that summary

            Each round is represented as:

            START EXAMPLE
            # player action
            *whatever I want {player} to do or say.*
            # gm command
            *whatever I want you to respect while generating the next message.*
            # assistant output
            * the output that was provided by the assistant*
            # secret info
            * The secret Info generated for the output*
            END EXAMPLE

            Rounds are separated by the delimiter:
            ---

            TASK:

            - Produce an updated summary that incorporates all rounds since the previous summary.
              Or create a new one, if there is none to update. Keep the summary as concicse as
              possible. It may at most be 2000 words in size.

            RULES:

            - Do NOT continue the story.
            - Do NOT roleplay.
            - Do NOT respond as the storyteller.
            - Do NOT follow the storyteller output format.
            - Do NOT invent new events.
            - Do NOT address the player.

            OUTPUT REQUIREMENTS:

            - Output ONLY the updated summary.
            - Use concise, neutral, factual language.
            - Past tense.
            - Focus on:
              - Major plot developments
              - Important world or character state changes
              - Decisions and consequences that affect future gameplay
            - No formatting beyond plain paragraphs unless explicitly requested.
            - Make sure to include all relevant information from the last summary
              and all provided turns, don't overweight the latest ones.
            - You may drop the least important information to keep the word-limit.

            You are a summarization tool, not a storyteller.
        "#};

    let term_strs = turns
        .iter()
        .map(|t| {
            let TurnInput {
                player_action,
                gm_instruction,
            } = &t.input;
            indoc::formatdoc! {
                "# player action
                {}
                # gm command
                {}
                # assistant output
                {}
                # secret info
                {}", player_action,
               gm_instruction,
               t.output.text,
               t.output.secret_info
            }
        })
        .collect::<Vec<_>>();

    let user_message = indoc::formatdoc! {r#"
            # Last Summary

            {last_summary}
                
            # Turns to summarize

            {}
        "#, term_strs.join("\n---\n")};

    debug!("Sending summary request - system msg:\n{system_message}");
    debug!("Sending summary request - user msg:\n{user_message}");
    let mut stream = llm.send_request_stream(Request {
        system: Some(system_message.into()),
        messages: vec![InputMessage::user(user_message)],
        max_tokens: 3000,
    });

    let response = loop {
        if let Some(m) = stream.try_next().await? {
            if let ResponseFragment::MessageComplete(m) = m {
                break m;
            }
        }
    };

    ensure!(matches!(stream.try_next().await, Ok(None)));
    Ok(response)
}

fn parse_image_description(src: &str) -> Result<ImageDescription> {
    let parts = src.split("<<<EOID>>>").collect::<Vec<&str>>();
    let [description, caption] = parts[..] else {
        bail!("No <<<EOID>>> in output");
    };

    Ok(ImageDescription {
        description: description.into(),
        caption: caption.into(),
    })
}

async fn get_image(
    rx_img_description: oneshot::Receiver<ImageDescription>,
    imgmod: ImgModBox,
) -> Result<Image> {
    let ImageDescription {
        description,
        caption,
    } = rx_img_description.await?;

    let image_model::Image { data, cost } = imgmod.get_image(&description).await?;

    Ok(Image {
        caption: caption,
        description,
        cost,
        jpeg_bytes: data,
    })
}

pub enum StartResultOrData {
    StartResult(AdvanceResult, TurnInput),
    Data(TurnData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameData {
    pub world_description: WorldDescription,
    pub pc: String,
    pub summaries: Vec<Summary>,
    pub turn_data: Vec<TurnData>,
}

impl GameData {
    fn construct_request(&self, input: &TurnInput, image_gen_extra_infos: &str) -> Request {
        let player = &self.pc;
        let world_description = &self.world_description.main_description;
        let pc_description = &self.world_description.pc_descriptions[&self.pc];
        let last_summary = self.summaries.last();
        let (summary, summary_turn) = match last_summary {
            Some(Summary { content, bday }) => (content.as_str(), *bday),
            None => ("", 0),
        };

        let system_message = indoc::formatdoc! {r#"
           You are a Story-teller-game. Below, I will provide a world-description.
           In that world, I control {player}. When I input anything, it's a command
           for {player} to execute in the world. Then it's your turn to decide and tell
           me how the world react to my input, and what happens. One pair of messages,
           one from me + one from you is called a turn.

           For each turn, you will also
           generate a description that can be passed to an image model to generate an image.
           When describing characters in image descriptions use many details, and make
           sure they match the actual current character state to increase
           the likelyhood of characters looking the same everytime. Be consistent and
           precise, especially about hair style, clothes and accesories.
           {image_gen_extra_infos}
           
           
           My input will be structured like this: The turn number, followed by
           three sections, all optional, like this:

           --- START EXAMPLE ---
            turn *N*
            # player action
            *whatever I want {player} to do or say.*
            # gm command
            *whatever I want you to respect while generating the next message.*
           --- END EXAMPLE ---

           The player action means: what ever I write here is what {player} does or says.
           When {player} is in a
           state that doesn't allow my inputted action, or makes it
           implausible, modify it by the least amount required to be possible,
           or interprete it in a way that makes it possible. These actions can fail.

           The gm command means that I want control over the story, and you should
           respect it to the best of your abilities. 

           If I provide neither of those, it just means you should generate more output for the
           previous input.

           The output should have the following structure:
           --- START EXAMPLE ---
           *The image description* will be passed to an image model to generate an image.
           Unless the most important part of the current turn is a special place, scenery or
           object, the image should show a character that is currently important.
           <<<EOID>>>
           *A short image caption* will be displayed below the image 1-5 words
           <<<EOIC>>>
           *The output*: text that is displayed to me, this should be between 300 and 1500 words at most
           No need for characters to hold endless monologues.
           <<<EOO>>>
           *Secret info*:. Stuff that is related to output, but hidden from me,
           it's a note for yourself. Keep it real short 500 words at most. Don't repeat
           information here that's already in the inputs or outputs. Use to track relevant
           events that are not in the current scene, to note down hidden intentions, or plan
           for future turns.
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
           should NOT start with `The output*:`, additionally, it should not have a heading
           or the turn number.

           The Proposed Actions should be one sentence each, describing 3 different
           plausable next actions for {player} to take. They should not be prefixed
           with "Proposed action: " or anything else.

           Here is the description of the world the story plays in, and some some
           instructions about the style:
           --- START DESCRIPTION ---
           {world_description} 
           --- END DESCRIPTION ---

           Here is a description of my character, {player}:
           --- START DESCRIPTION ---
           {pc_description}
           --- END DESCRIPTION ---
           

           Here is a summary of everthing that has happened up till turn {summary_turn}:
           --- START SUMMARY ---
           {summary} 
           --- END SUMMARY ---

           Make sure you respect the output-length limitation of at most 1000 words.
        "#};

        let messages = (0..self.turn_data.len())
            .rev()
            .take(HIST_SIZE)
            .rev()
            .flat_map(|i| {
                let mut user_message = format!("turn {i}");
                let TurnData { input, output, .. } = &self.turn_data[i];
                input.write_to_user_msg_string(&mut user_message);
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
            max_tokens: 5000,
            system: Some(system_message),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub content: String,
    /// the turn after which it was created
    pub bday: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnData {
    pub summary_before_input: Option<usize>,
    pub input: TurnInput,
    pub output: TurnOutput,
    pub image_ids: NonEmpty<usize>,
    pub image_captions: NonEmpty<String>,
}

#[derive(Debug, Clone)]
pub struct Image {
    pub caption: String,
    pub description: String,
    pub cost: Option<f64>,
    pub jpeg_bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ImageDescription {
    pub description: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnOutput {
    pub text: String,
    pub image_description: String,
    pub image_caption: String,
    pub secret_info: String,
    pub proposed_next_actions: [String; N_PROPOSED_OPTIONS],
    pub input_tokens: usize,
    pub output_tokens: usize,
}

impl TurnOutput {
    fn to_llm_format(&self) -> String {
        let mut output = String::new();

        output.push_str(&self.image_description);
        output.push_str("\n<<<EOID>>>\n");
        output.push_str(&self.image_caption);
        output.push_str("\n<<<EOIC>>>\n");

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
        let parts = value.text.split("<<<EOID>>>").collect::<Vec<&str>>();
        let [image_description, tail] = parts[..] else {
            bail!("no <<<EOID>>> in output");
        };

        let parts = tail.split("<<<EOIC>>>").collect::<Vec<&str>>();
        let [image_caption, tail] = parts[..] else {
            bail!("no <<<EOIC>>> in output");
        };

        let parts = tail.split("<<<EOO>>>").collect::<Vec<&str>>();
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
            image_description: image_description.trim().into(),
            image_caption: image_caption.trim().into(),
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
pub struct TurnInput {
    pub player_action: String,
    pub gm_instruction: String,
}

impl TurnInput {
    pub fn player_action(s: String) -> Self {
        Self {
            player_action: s,
            gm_instruction: "".into(),
        }
    }

    pub fn write_to_user_msg_string(&self, user_message: &mut String) {
        user_message.push_str("\n# player action\n");
        user_message.push_str(&self.player_action);
        user_message.push_str("\n# gm command\n");
        user_message.push_str(&self.gm_instruction);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldDescription {
    pub name: String,
    pub main_description: String,
    pub pc_descriptions: BTreeMap<String, String>,
    pub init_action: String,
}
