//! This module contains the state machine that consumes the streamed LLM output.
//! It extracts the image description, forwards visible text as it arrives, and
//! finally emits the parsed turn output.

use color_eyre::{
    Result,
    eyre::Context,
};
use log::error;

use crate::llm::OutputMessage;

use super::{
    IMAGE_CAPTION_ENDS, IMAGE_DESCRIPTION, OUTPUT_STOPS, ImageDescription, ResponseFragment,
    SendToLLMState, StreamFinder, TurnOutput, parse_image_description,
    stream_finder::MatchResult,
};

pub(super) struct TurnStreamProcessor {
    mode: SendToLLMState,
    soi_finder: StreamFinder,
    eoi_finder: StreamFinder,
    eoo_finder: StreamFinder,
    discarded_prefix: String,
    image_description: String,
    received_text: String,
}

pub(super) enum ProcessorEvent {
    VisibleText(String),
    ImageDescriptionReady(ImageDescription),
    TurnComplete(TurnOutput),
}

impl TurnStreamProcessor {
    pub(super) fn new() -> Self {
        Self {
            mode: SendToLLMState::LookingForStartOfImageDescription,
            soi_finder: StreamFinder::new(IMAGE_DESCRIPTION),
            eoi_finder: StreamFinder::new(IMAGE_CAPTION_ENDS),
            eoo_finder: StreamFinder::new(OUTPUT_STOPS),
            discarded_prefix: String::new(),
            image_description: String::new(),
            received_text: String::new(),
        }
    }

    pub(super) fn push(&mut self, fragment: ResponseFragment) -> Result<Vec<ProcessorEvent>> {
        match fragment {
            ResponseFragment::TextDelta(f) => {
                self.received_text.push_str(&f);
                self.push_text_delta(f)
            }
            ResponseFragment::MessageComplete(m) => self.finish_message(m),
        }
    }

    pub(super) fn status_summary(&self) -> String {
        format!(
            "mode={:?}, discarded_prefix_len={}, image_description_len={}, received_text_len={}",
            self.mode,
            self.discarded_prefix.len(),
            self.image_description.len(),
            self.received_text.len(),
        )
    }

    pub(super) fn received_text(&self) -> &str {
        &self.received_text
    }

    fn push_text_delta(&mut self, text: String) -> Result<Vec<ProcessorEvent>> {
        let mut events = Vec::new();
        let mut rest = text;

        while !rest.is_empty() {
            rest = match self.mode {
                SendToLLMState::LookingForStartOfImageDescription => {
                    self.handle_looking_for_start(rest)
                }
                SendToLLMState::ParsingImageDescription => {
                    self.handle_parsing_image_description(rest, &mut events)?
                }
                SendToLLMState::StreamingOutputText => {
                    self.handle_streaming_output(rest, &mut events)
                }
                SendToLLMState::FinishingUp => String::new(),
            };
        }

        Ok(events)
    }

    fn finish_message(&mut self, message: OutputMessage) -> Result<Vec<ProcessorEvent>> {
        let output = TurnOutput::try_from(message).context("parse output")?;
        Ok(vec![ProcessorEvent::TurnComplete(output)])
    }

    fn handle_looking_for_start(&mut self, fragment: String) -> String {
        match self.soi_finder.process(&fragment) {
            MatchResult::Blocked => String::new(),
            MatchResult::StopTokenMatched {
                pre_token_text,
                post_token_text,
            } => {
                self.discarded_prefix.push_str(&pre_token_text);
                self.mode = SendToLLMState::ParsingImageDescription;
                post_token_text
            }
            MatchResult::CheckedOutput(output) => {
                self.discarded_prefix.push_str(&output);
                String::new()
            }
        }
    }

    fn handle_parsing_image_description(
        &mut self,
        fragment: String,
        events: &mut Vec<ProcessorEvent>,
    ) -> Result<String> {
        let rest = match self.eoi_finder.process(&fragment) {
            MatchResult::Blocked => String::new(),
            MatchResult::CheckedOutput(output) => {
                self.image_description.push_str(&output);
                String::new()
            }
            MatchResult::StopTokenMatched {
                pre_token_text,
                post_token_text,
            } => {
                self.image_description.push_str(&pre_token_text);
                self.mode = SendToLLMState::StreamingOutputText;
                let description = parse_image_description(&self.image_description)
                    .inspect_err(|e| {
                        error!(
                            "Failed to parse streamed LLM image prefix:\n{}\nParse error: {e:?}",
                            self.image_description,
                        )
                    })
                    .context("parsing image description")?;
                events.push(ProcessorEvent::ImageDescriptionReady(description));
                post_token_text
            }
        };

        Ok(rest)
    }

    fn handle_streaming_output(
        &mut self,
        fragment: String,
        events: &mut Vec<ProcessorEvent>,
    ) -> String {
        match self.eoo_finder.process(&fragment) {
            MatchResult::Blocked => String::new(),
            MatchResult::CheckedOutput(output) => {
                events.push(ProcessorEvent::VisibleText(output));
                String::new()
            }
            MatchResult::StopTokenMatched {
                pre_token_text: processed,
                post_token_text: _,
            } => {
                if !processed.is_empty() {
                    events.push(ProcessorEvent::VisibleText(processed));
                }
                self.mode = SendToLLMState::FinishingUp;
                String::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::llm::OutputMessage;

    use super::*;

    fn text_delta(s: &str) -> ResponseFragment {
        ResponseFragment::TextDelta(s.to_string())
    }

    #[test]
    fn emits_image_description_when_caption_marker_arrives() {
        let mut processor = TurnStreamProcessor::new();
        let events = processor
            .push(text_delta(
                "preface [[[IMAGE DESCRIPTION]]]\nportrait details\n[[[IMAGE DESCRIPTION STOPS]]]\nNight Watch\n[[[IMAGE CAPTION ENDS]]]",
            ))
            .unwrap();

        assert_eq!(events.len(), 1);
        let ProcessorEvent::ImageDescriptionReady(desc) = &events[0] else {
            panic!("expected image description event");
        };
        assert_eq!(desc.description, "portrait details");
        assert_eq!(desc.caption, "Night Watch");
    }

    #[test]
    fn emits_visible_text_after_caption_marker() {
        let mut processor = TurnStreamProcessor::new();
        let events = processor
            .push(text_delta(
                "[[[IMAGE DESCRIPTION]]]\nportrait details\n[[[IMAGE DESCRIPTION STOPS]]]\nNight Watch\n[[[IMAGE CAPTION ENDS]]]\nVisible intro",
            ))
            .unwrap();

        assert_eq!(events.len(), 2);
        let ProcessorEvent::VisibleText(text) = &events[1] else {
            panic!("expected visible text event");
        };
        assert_eq!(text, "\nVisible intro");
    }

    #[test]
    fn ignores_text_after_output_stop_until_completion() {
        let mut processor = TurnStreamProcessor::new();
        let _ = processor
            .push(text_delta(
                "[[[IMAGE DESCRIPTION]]]\nportrait\n[[[IMAGE DESCRIPTION STOPS]]]\nNight Watch\n[[[IMAGE CAPTION ENDS]]]\nShown text[[[OUTPUT STOPS]]]\nsecret",
            ))
            .unwrap();

        let events = processor
            .push(ResponseFragment::MessageComplete(OutputMessage {
                text: "[[[IMAGE DESCRIPTION]]]\nportrait\n[[[IMAGE DESCRIPTION STOPS]]]\nNight Watch\n[[[IMAGE CAPTION ENDS]]]\nShown text[[[OUTPUT STOPS]]]\nsecret[[[SECRET STOPS]]]\na1[[[ACTION BREAK]]]a2[[[ACTION BREAK]]]a3".into(),
                input_tokens: 1,
                output_tokens: 1,
            }))
            .unwrap();

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], ProcessorEvent::TurnComplete(_)));
    }
}
