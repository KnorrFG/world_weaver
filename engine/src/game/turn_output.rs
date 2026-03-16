//! `TurnOutput` is the structured form of a fully generated turn.
//! This module also contains the parser and serializer for the custom output format.

use color_eyre::eyre::eyre;
use log::{error, warn};
use serde::{Deserialize, Serialize};

use crate::{
    N_PROPOSED_OPTIONS,
    llm::OutputMessage,
};

use super::{
    ACTION_BREAK, IMAGE_CAPTION_ENDS, IMAGE_DESCRIPTION, IMAGE_DESCRIPTION_STOPS, OUTPUT_STOPS,
    SECRET_STOPS,
};

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
    pub fn from_parts(
        image_description: String,
        image_caption: String,
        text: String,
        secret_info: Option<String>,
        proposed_next_actions: Vec<String>,
        input_tokens: usize,
        output_tokens: usize,
    ) -> Self {
        let mut actions = proposed_next_actions
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        actions.resize(N_PROPOSED_OPTIONS, "missing".into());

        Self {
            image_description: image_description.trim().into(),
            image_caption: image_caption.trim().into(),
            text: text.trim().into(),
            secret_info: fallback_if_empty(
                secret_info
                    .unwrap_or_else(|| "none".into())
                    .trim()
                    .to_string(),
                "none",
            ),
            proposed_next_actions: actions[..N_PROPOSED_OPTIONS].to_vec().try_into().unwrap(),
            input_tokens,
            output_tokens,
        }
    }

    pub fn to_llm_format(&self) -> String {
        let mut output = String::new();

        output.push_str("\n");
        output.push_str(IMAGE_DESCRIPTION);
        output.push_str("\n");
        output.push_str(&self.image_description);
        output.push_str("\n");
        output.push_str(IMAGE_DESCRIPTION_STOPS);
        output.push_str("\n");
        output.push_str(&self.image_caption);
        output.push_str("\n");
        output.push_str(IMAGE_CAPTION_ENDS);
        output.push_str("\n");

        output.push_str(&self.text);

        output.push_str("\n");
        output.push_str(OUTPUT_STOPS);
        output.push_str("\n");
        output.push_str(&self.secret_info);
        output.push_str("\n");
        output.push_str(SECRET_STOPS);
        output.push_str("\n");
        output.push_str(&self.proposed_next_actions.join(&format!("\n{ACTION_BREAK}\n")));

        output
    }
}

impl TryFrom<OutputMessage> for TurnOutput {
    type Error = color_eyre::Report;

    fn try_from(value: OutputMessage) -> std::result::Result<Self, Self::Error> {
        let parts = value.text.split(IMAGE_DESCRIPTION).collect::<Vec<&str>>();
        let Some(tail) = parts.last().copied() else {
            let err = eyre!("impossible?");
            error!("Failed to parse LLM message:\n{}\nParse error: {err:?}", value.text);
            return Err(err);
        };
        let parts = tail.split(IMAGE_DESCRIPTION_STOPS).collect::<Vec<&str>>();
        let [image_description, tail] = parts[..] else {
            let err = eyre!("no {IMAGE_DESCRIPTION_STOPS} in output");
            error!("Failed to parse LLM message:\n{}\nParse error: {err:?}", value.text);
            return Err(err);
        };

        let parts = tail.split(IMAGE_CAPTION_ENDS).collect::<Vec<&str>>();
        let [image_caption, tail] = parts[..] else {
            let err = eyre!("no {IMAGE_CAPTION_ENDS} in output");
            error!("Failed to parse LLM message:\n{}\nParse error: {err:?}", value.text);
            return Err(err);
        };

        let parts = tail.split(OUTPUT_STOPS).collect::<Vec<&str>>();
        let [output, tail] = parts[..] else {
            let err = eyre!("No {OUTPUT_STOPS} in output");
            error!("Failed to parse LLM message:\n{}\nParse error: {err:?}", value.text);
            return Err(err);
        };

        let parts = tail.split(SECRET_STOPS).collect::<Vec<&str>>();
        let (secret, action_text) = if parts.len() == 1 {
            (None, parts[0])
        } else {
            (Some(parts[0].to_string()), parts[1])
        };

        let proposed_next_actions = action_text
            .split(ACTION_BREAK)
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>();

        if proposed_next_actions.iter().filter(|s| !s.is_empty()).count() < N_PROPOSED_OPTIONS {
            warn!(
                "Incomplete output tail, filling defaults. Found {} proposed actions.",
                proposed_next_actions.iter().filter(|s| !s.is_empty()).count(),
            );
        }

        Ok(TurnOutput::from_parts(
            image_description.into(),
            image_caption.into(),
            output.into(),
            secret,
            proposed_next_actions,
            value.input_tokens,
            value.output_tokens,
        ))
    }
}

fn fallback_if_empty(s: String, fallback: &str) -> String {
    if s.is_empty() { fallback.into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_turn_output_from_marker_format() {
        let raw = r#"
ignored prefix
[[[IMAGE DESCRIPTION]]]
hero portrait
[[[IMAGE DESCRIPTION STOPS]]]
Night Watch
[[[IMAGE CAPTION ENDS]]]
You step into the alley.
[[[OUTPUT STOPS]]]
The watcher is armed.
[[[SECRET STOPS]]]
Move closer.
[[[ACTION BREAK]]]
Hide behind crates.
[[[ACTION BREAK]]]
Call out softly.
"#;

        let parsed = TurnOutput::try_from(OutputMessage {
            text: raw.into(),
            input_tokens: 12,
            output_tokens: 34,
        })
        .unwrap();

        assert_eq!(parsed.image_description, "hero portrait");
        assert_eq!(parsed.image_caption, "Night Watch");
        assert_eq!(parsed.text, "You step into the alley.");
        assert_eq!(parsed.secret_info, "The watcher is armed.");
        assert_eq!(
            parsed.proposed_next_actions,
            [
                String::from("Move closer."),
                String::from("Hide behind crates."),
                String::from("Call out softly.")
            ]
        );
    }

    #[test]
    fn fills_missing_secret_and_actions_with_defaults() {
        let raw = r#"
[[[IMAGE DESCRIPTION]]]
hero portrait
[[[IMAGE DESCRIPTION STOPS]]]
Night Watch
[[[IMAGE CAPTION ENDS]]]
You step into the alley.
[[[OUTPUT STOPS]]]
"#;

        let parsed = TurnOutput::try_from(OutputMessage {
            text: raw.into(),
            input_tokens: 12,
            output_tokens: 34,
        })
        .unwrap();

        assert_eq!(parsed.secret_info, "none");
        assert_eq!(
            parsed.proposed_next_actions,
            [
                String::from("missing"),
                String::from("missing"),
                String::from("missing")
            ]
        );
    }
}
