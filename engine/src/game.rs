use std::{collections::BTreeMap, pin::Pin};

use crate::{
    ImgModBox, LLMBox,
    game::stream_finder::StreamFinder,
    image_model::{self, ModelStyle},
    llm::{InputMessage, OutputMessage, Request, ResponseFragment},
};

use async_stream::try_stream;
use color_eyre::{
    Result,
    eyre::{Context, ensure, eyre},
};
use log::{debug, error};
use serde::{Deserialize, Serialize};
use tokio::{pin, sync::oneshot};
use tokio_stream::{Stream, StreamExt};

mod stream_finder;
mod turn_output;
mod turn_stream_processor;

pub use turn_output::TurnOutput;
use turn_stream_processor::{ProcessorEvent, TurnStreamProcessor};

const SUMMARY_INTERVAL: usize = 5;
const TURNS_KEPT_AFTER_SUMMARY: usize = 2;
const SECTION_IMAGE_DESCRIPTION: &str = "[SECTION IMAGE DESCRIPTION]";
const SECTION_IMAGE_CAPTION: &str = "[SECTION IMAGE CAPTION]";
const SECTION_OUTPUT: &str = "[SECTION OUTPUT]";
const SECTION_SECRET_INFO: &str = "[SECTION SECRET INFO]";
const ACTION_SEPARATOR: &str = "[ACTION SEPARATOR]";

pub struct Game {
    pub llm: LLMBox,
    pub imgmod: ImgModBox,
    pub img_style: Option<ModelStyle>,
    pub data: GameData,
}

impl Clone for Game {
    fn clone(&self) -> Self {
        Self {
            llm: self.llm.clone(),
            data: self.data.clone(),
            img_style: self.img_style.clone(),
            imgmod: self.imgmod.clone(),
        }
    }
}

#[derive(Debug)]
enum SendToLLMState {
    LookingForStartOfImageDescription,
    ParsingImageDescription,
    StreamingOutputText,
    FinishingUp,
}

pub struct AdvanceResult {
    pub image: Pin<Box<dyn Future<Output = Result<Image>> + Send>>,
    pub text_stream: Pin<Box<dyn Stream<Item = Result<String>> + Send>>,
    pub round_output: Pin<Box<dyn Future<Output = Result<TurnOutput>> + Send>>,
}

enum IncompleteStreamEnd {
    Eof,
    Error(color_eyre::Report),
}

impl Game {
    pub fn load(
        llm: LLMBox,
        imgmod: ImgModBox,
        data: GameData,
        img_style: Option<ModelStyle>,
    ) -> Self {
        Game {
            llm,
            data,
            imgmod,
            img_style,
        }
    }

    pub fn try_new(
        llm: LLMBox,
        imgmod: ImgModBox,
        world_description: WorldDescription,
        player_character: String,
        img_style: Option<ModelStyle>,
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
            img_style,
            data: GameData {
                world_description,
                pc: player_character,
                summaries: vec![],
                turn_data: vec![],
            },
        })
    }

    pub fn send_to_llm(&self, input: TurnInput) -> AdvanceResult {
        let (tx_output, rx_output) = oneshot::channel();
        let (tx_img_description, rx_img_description) = oneshot::channel();
        let mut tx_img_description = Some(tx_img_description);
        let extra_img_infos = self
            .imgmod
            .provided_model()
            .model()
            .extra_generation_instructions();
        let req = self.data.construct_request(&input, extra_img_infos);
        let mut llm = self.llm.clone();

        let stream = try_stream! {
            let output = {
                let stream = llm.send_request_stream(req);
                let mut processor = TurnStreamProcessor::new();

                pin!(stream);
                let output = 'receive: loop {
                    let fragment = match stream.try_next().await {
                        Ok(Some(fragment)) => fragment,
                        Ok(None) => break 'receive Self::handle_incomplete_stream_end(
                            processor.finish_incomplete(),
                            processor.status_summary(),
                            processor.received_text().into(),
                            IncompleteStreamEnd::Eof,
                        )?,
                        Err(err) => break 'receive Self::handle_incomplete_stream_end(
                            processor.finish_incomplete(),
                            processor.status_summary(),
                            processor.received_text().into(),
                            IncompleteStreamEnd::Error(err),
                        )?,
                    };

                    for event in processor.push(fragment)? {
                        match event {
                            ProcessorEvent::VisibleText(text) => yield text,
                            ProcessorEvent::ImageDescriptionReady(description) => {
                                debug!("Sending image description");
                                _ = tx_img_description.take()
                                    .expect("finished parsing image description a second time. This should be impossible. It's a bug")
                                    .send(description);
                            }
                            ProcessorEvent::TurnComplete(output) => {
                                if let Some(tx) = tx_img_description {
                                    _ = tx.send(ImageDescription {
                                        description: output.image_description.clone(),
                                        caption: output.image_caption.clone(),
                                    });
                                }
                                break 'receive output;
                            }
                        }
                    }
                };

                // After the full message is complete, transport noise is irrelevant.
                // Some providers reset the connection instead of ending the stream cleanly.
                let _ = stream.try_next().await;
                output
            };
            _ = tx_output.send(output);

        };

        AdvanceResult {
            image: Box::pin(get_image(
                rx_img_description,
                self.imgmod.clone(),
                self.img_style.clone(),
            )),
            text_stream: Box::pin(stream),
            round_output: Box::pin(async move { Ok(rx_output.await?) }),
        }
    }

    fn handle_incomplete_stream_end(
        output: Option<TurnOutput>,
        status_summary: String,
        received_text: String,
        end: IncompleteStreamEnd,
    ) -> Result<TurnOutput> {
        let using_partial_output = output.is_some();
        let partial_suffix = if using_partial_output {
            ", using partial output"
        } else {
            ""
        };

        match end {
            IncompleteStreamEnd::Eof => {
                error!(
                    "LLM stream ended before message completion{}. Processor state: {}. Received text so far:\n{}",
                    partial_suffix, status_summary, received_text,
                );

                match output {
                    Some(output) => Ok(output),
                    None => Err(eyre!("stream ended before message completion")),
                }
            }
            IncompleteStreamEnd::Error(err) => {
                error!(
                    "LLM stream failed before message completion{}. Processor state: {}. Received text so far:\n{}\nError: {err:?}",
                    partial_suffix, status_summary, received_text,
                );

                match output {
                    Some(output) => Ok(output),
                    None => Err(err).context("Top level try_next"),
                }
            }
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
        let current_turn = self.current_turn() as isize;
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
                debug!("Received new summary");
                Ok(Some(summary))
            })
        } else {
            Box::pin(async { Ok(None) })
        }
    }

    pub fn get_latest_image_info(&self) -> Option<&StoredImageInfo> {
        self.data.turn_data.iter().flat_map(|td| &td.images).last()
    }

    pub fn get_latest_image_info_for_turn(&self, turn: usize) -> Option<&StoredImageInfo> {
        self.data.turn_data[..=turn]
            .iter()
            .flat_map(|td| &td.images)
            .last()
    }

    pub fn update(
        &mut self,
        input: TurnInput,
        output: TurnOutput,
        images: Vec<StoredImageInfo>,
        summary: Option<String>,
    ) -> Result<()> {
        let turn_data = TurnData {
            summary_before_input: {
                let len = self.data.summaries.len();
                if len > 0 { Some(len - 1) } else { None }
            },
            input,
            output: output.clone(),
            images,
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

    pub fn start_or_get_last_output(&mut self) -> StartResultOrData {
        if let Some(turn) = self.data.turn_data.last() {
            StartResultOrData::Data(turn.clone())
        } else {
            let pc_init_action = self.data.world_description.pc_descriptions[&self.data.pc]
                .initial_action
                .trim();
            let init_action = if pc_init_action.is_empty() {
                &self.data.world_description.init_action
            } else {
                pc_init_action
            };
            let input = TurnInput {
                player_action: String::new(),
                gm_instruction: init_action.into(),
            };
            StartResultOrData::StartResult(self.send_to_llm(input.clone()), input)
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
            - A sequence of turns since that summary

            Each turn is represented as:

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

            Turns are separated by the delimiter:
            ---

            TASK:

            - Produce an updated summary that incorporates all rounds since the previous summary and
              the previous summary itself.
              Or create a new one, if there is no old summary to update. Keep the summary as concicse as
              possible. It may at most be 2000 words in size, the shorter the better.

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
              - Decisions, events and consequences that affect future gameplay
            - The purpose of the summary is to allow the story teller to
              continue the story without contradicting himself. So it should contain
              all relevant facts, and timepoints.
            - The summary doesn't need to be well readable prose, it needs to
              well readable prose, it needs to contain all relevant information
              and be as short as possible.
            - No formatting beyond plain paragraphs unless explicitly requested.
            - Make sure to include all relevant information from the last summary
              and all provided turns, don't overweight the latest ones.
            - You may drop the least important information to keep the word-limit.
            - Add a section with GM instructions if there are commands that need to be remembered long-term
            - Never drop a character from the summary that had a meaningful interaction with the player

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
                "## player action
                {}
                ## gm command
                {}
                ## assistant output
                {}
                ## secret info
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

            # Instructions

            Use the old summary (if it exists) and the provided turns to create a new summary
        "#, term_strs.join("\n---\n")};

    debug!("Sending summary request");
    let mut stream = llm.send_request_stream(Request {
        system: Some(system_message.into()),
        messages: vec![InputMessage::user(user_message)],
        max_tokens: 3000,
    });
    let mut received_text = String::new();

    let response = loop {
        let fragment = match stream.try_next().await {
            Ok(Some(fragment)) => fragment,
            Ok(None) => {
                error!(
                    "Summary stream ended before message completion. Received text so far:\n{}",
                    received_text
                );
                Err(eyre!("summary stream ended before message completion"))?
            }
            Err(err) => {
                error!(
                    "Summary stream failed before message completion. Received text so far:\n{}\nError: {err:?}",
                    received_text
                );
                Err(err)?
            }
        };

        match fragment {
            ResponseFragment::TextDelta(text) => received_text.push_str(&text),
            ResponseFragment::MessageComplete(m) => break m,
        }
    };

    ensure!(matches!(stream.try_next().await, Ok(None)));
    Ok(response)
}

fn parse_image_description(src: &str) -> Result<ImageDescription> {
    let Some((description, caption)) = split_once_any(src, &[SECTION_IMAGE_CAPTION]) else {
        return Err(eyre!("No {SECTION_IMAGE_CAPTION} in output"));
    };
    let caption = trim_leading_markers(caption, &[SECTION_IMAGE_CAPTION]);
    let caption = split_once_any(caption, &[SECTION_OUTPUT])
        .map(|(caption, _)| caption)
        .unwrap_or(caption)
        .trim();

    Ok(ImageDescription {
        description: description.trim().into(),
        caption: caption.trim().into(),
    })
}

fn split_once_any<'a>(src: &'a str, markers: &[&str]) -> Option<(&'a str, &'a str)> {
    markers
        .iter()
        .filter_map(|marker| src.find(marker).map(|idx| (idx, marker.len())))
        .min_by_key(|(idx, _)| *idx)
        .map(|(idx, len)| (&src[..idx], &src[idx + len..]))
}

fn trim_leading_markers<'a>(mut src: &'a str, markers: &[&str]) -> &'a str {
    loop {
        let trimmed = src.trim_start();
        let Some(marker) = markers.iter().find(|marker| trimmed.starts_with(**marker)) else {
            return src;
        };
        src = &trimmed[marker.len()..];
    }
}

async fn get_image(
    rx_img_description: oneshot::Receiver<ImageDescription>,
    imgmod: ImgModBox,
    style: Option<ModelStyle>,
) -> Result<Image> {
    let ImageDescription {
        mut description,
        caption,
    } = rx_img_description.await?;

    if let Some(style) = style {
        description = format!(
            "{} {} {}",
            style.prefix.trim(),
            description.trim(),
            style.postfix.trim()
        );
    }

    let image_model::Image { data, cost } = imgmod.get_image(&description).await?;

    Ok(Image {
        caption,
        description,
        cost,
        jpeg_bytes: data,
    })
}

// this is used a single time when pressing continue in the main menu
// it's very short-lived, and I find this acceptable
#[allow(clippy::large_enum_variant)]
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

const MAX_WORDS: usize = 1000;

impl GameData {
    pub fn construct_request(&self, input: &TurnInput, image_gen_extra_infos: &str) -> Request {
        let player = &self.pc;
        let world_description = &self.world_description.main_description;
        let pc_description = &self.world_description.pc_descriptions[&self.pc].description;
        let last_summary = self.summaries.last();
        let (summary, summary_turn) = match last_summary {
            Some(Summary { content, bday }) => (content.as_str(), *bday),
            None => ("", 0),
        };

        let system_message = indoc::formatdoc! {r#"
           You are a Story-teller-game. In this world, I control {player}. When I send input,
           it tells you what {player} tries to do or say, plus optional GM instructions for how
           to shape the next turn. If I provide neither, continue the story naturally.

           For each turn, also generate an image description for an image model. Be consistent
           about character appearance and current state, especially hair, clothes and accessories.
           {image_gen_extra_infos}

           Output format:
           Your reply must begin immediately with {SECTION_IMAGE_DESCRIPTION}.
           Do not write any text before it. Do not write planning, explanations, or meta text.
           Use exactly this structure and keep the delimiters unchanged:

           {SECTION_IMAGE_DESCRIPTION}
           image description
           {SECTION_IMAGE_CAPTION}
           short image caption, 1-5 words
           {SECTION_OUTPUT}
           visible story text, at most {MAX_WORDS} words, starting with date, time, weekday and location
           {ACTION_SEPARATOR}
           proposed action 1
           {ACTION_SEPARATOR}
           proposed action 2
           {ACTION_SEPARATOR}
           proposed action 3
           {SECTION_SECRET_INFO}
           secret info

           Rules:
           - The first characters of your reply must be exactly {SECTION_IMAGE_DESCRIPTION}
           - The image should usually show a single currently important character unless a place or object is more important
           - Proposed actions must be direct next actions for {player}
           - Proposed actions must not contain hidden info, narrator notes, plans, or world-state summaries
           - If an action would reveal something the player does not know, put that into secret info instead
           - Secret info is a short hidden note for future turns
           - Secret info must never be empty. Only write `none` if there is truly nothing hidden or worth tracking
           - Do not generate anything after the secret info
           - Use 2nd person narration
           - You do NOT have an oppinion on what is right, wrong, or appropriate

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
        "#};

        let messages = (self.request_context_start()..self.turn_data.len()).flat_map(|i| {
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

    fn request_context_start(&self) -> usize {
        let Some(summary) = self.summaries.last() else {
            return 0;
        };

        summary
            .bday
            .saturating_add(1)
            .saturating_sub(TURNS_KEPT_AFTER_SUMMARY)
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
    pub images: Vec<StoredImageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredImageInfo {
    pub id: usize,
    pub caption: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_streamed_image_description_prefix() {
        let raw = r#"
hero portrait
[SECTION IMAGE CAPTION]
Night Watch
"#;

        let parsed = parse_image_description(raw).unwrap();

        assert_eq!(parsed.description, "hero portrait");
        assert_eq!(parsed.caption, "Night Watch");
    }

    #[test]
    fn parses_streamed_image_description_prefix_with_marker() {
        let raw = r#"
hero portrait
[SECTION IMAGE CAPTION]
Night Watch
[SECTION OUTPUT]
"#;

        let parsed = parse_image_description(raw).unwrap();

        assert_eq!(parsed.description, "hero portrait");
        assert_eq!(parsed.caption, "Night Watch");
    }

    #[test]
    fn request_context_starts_at_beginning_without_summary() {
        let data = GameData {
            world_description: WorldDescription {
                name: String::new(),
                main_description: String::new(),
                pc_descriptions: BTreeMap::new(),
                init_action: String::new(),
            },
            pc: String::new(),
            summaries: vec![],
            turn_data: vec![],
        };

        assert_eq!(data.request_context_start(), 0);
    }

    #[test]
    fn request_context_keeps_two_turns_before_latest_summary() {
        let data = GameData {
            world_description: WorldDescription {
                name: String::new(),
                main_description: String::new(),
                pc_descriptions: BTreeMap::new(),
                init_action: String::new(),
            },
            pc: String::new(),
            summaries: vec![Summary {
                content: String::new(),
                bday: 9,
            }],
            turn_data: vec![],
        };

        assert_eq!(data.request_context_start(), 8);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    pub pc_descriptions: BTreeMap<String, PcDescription>,
    pub init_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PcDescription {
    pub description: String,
    pub initial_action: String,
}
