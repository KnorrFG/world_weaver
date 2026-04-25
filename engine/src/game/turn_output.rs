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
    ACTION_SEPARATOR, SECTION_IMAGE_CAPTION, SECTION_IMAGE_DESCRIPTION, SECTION_OUTPUT,
    SECTION_SECRET_INFO,
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

        output.push('\n');
        output.push_str(SECTION_IMAGE_DESCRIPTION);
        output.push('\n');
        output.push_str(&self.image_description);
        output.push('\n');
        output.push_str(SECTION_IMAGE_CAPTION);
        output.push('\n');
        output.push_str(&self.image_caption);
        output.push('\n');
        output.push_str(SECTION_OUTPUT);
        output.push('\n');

        output.push_str(&self.text);

        output.push('\n');
        output.push_str(ACTION_SEPARATOR);
        output.push('\n');
        output.push_str(&self.proposed_next_actions.join(&format!("\n{ACTION_SEPARATOR}\n")));
        output.push('\n');
        output.push_str(SECTION_SECRET_INFO);
        output.push('\n');
        output.push_str(&self.secret_info);

        output
    }
}

impl TryFrom<OutputMessage> for TurnOutput {
    type Error = color_eyre::Report;

    fn try_from(value: OutputMessage) -> std::result::Result<Self, Self::Error> {
        let Some((_, tail)) = split_once_any(&value.text, &[SECTION_IMAGE_DESCRIPTION]) else {
            let err = eyre!("no {SECTION_IMAGE_DESCRIPTION} in output");
            error!("Failed to parse LLM message:\n{}\nParse error: {err:?}", value.text);
            return Err(err);
        };
        let Some((image_description, tail)) =
            split_once_any(tail, &[SECTION_IMAGE_CAPTION])
        else {
            let err = eyre!("no {SECTION_IMAGE_CAPTION} in output");
            error!("Failed to parse LLM message:\n{}\nParse error: {err:?}", value.text);
            return Err(err);
        };
        let tail = trim_leading_markers(tail, &[SECTION_IMAGE_CAPTION]);

        let Some((image_caption, tail)) = split_once_any(tail, &[SECTION_OUTPUT]) else {
            let err = eyre!("no {SECTION_OUTPUT} in output");
            error!("Failed to parse LLM message:\n{}\nParse error: {err:?}", value.text);
            return Err(err);
        };

        let Some((output, tail)) = split_once_any(tail, &[ACTION_SEPARATOR]) else {
            let err = eyre!("No {ACTION_SEPARATOR} in output");
            error!("Failed to parse LLM message:\n{}\nParse error: {err:?}", value.text);
            return Err(err);
        };

        let (action_text, secret) = if let Some((action_text, secret)) =
            split_once_any(tail, &[SECTION_SECRET_INFO])
        {
            (action_text, Some(secret.to_string()))
        } else {
            (tail, None)
        };

        let proposed_next_actions = action_text
            .split(ACTION_SEPARATOR)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_turn_output_from_marker_format() {
        let raw = r#"
ignored prefix
[SECTION IMAGE DESCRIPTION]
hero portrait
[SECTION IMAGE CAPTION]
Night Watch
[SECTION OUTPUT]
You step into the alley.
[ACTION SEPARATOR]
Move closer.
[ACTION SEPARATOR]
Hide behind crates.
[ACTION SEPARATOR]
Call out softly.
[SECTION SECRET INFO]
The watcher is armed.
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
[SECTION IMAGE DESCRIPTION]
hero portrait
[SECTION IMAGE CAPTION]
Night Watch
[SECTION OUTPUT]
You step into the alley.
[ACTION SEPARATOR]
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

    #[test]
    fn tolerates_duplicate_markers() {
        let raw = r#"
[SECTION IMAGE DESCRIPTION]
hero portrait
[SECTION IMAGE CAPTION]
[SECTION IMAGE CAPTION]
Night Watch
[SECTION OUTPUT]
You step into the alley.
[ACTION SEPARATOR]
Move closer.
[ACTION SEPARATOR]
Hide behind crates.
[ACTION SEPARATOR]
Call out softly.
[SECTION SECRET INFO]
The watcher is armed.
"#;

        let parsed = TurnOutput::try_from(OutputMessage {
            text: raw.into(),
            input_tokens: 12,
            output_tokens: 34,
        })
        .unwrap();

        assert_eq!(parsed.image_caption, "Night Watch");
        assert_eq!(parsed.secret_info, "The watcher is armed.");
    }
}
