//! `TurnOutput` is the structured form of a fully generated turn.
//! This module also contains the parser and serializer for the custom output format.

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
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

        let res: Result<Self> = (|| {
            let parts = tail.split(SECRET_STOPS).collect::<Vec<&str>>();
            let (secret, tail) = if parts.len() == 1 {
                ("", parts[0])
            } else {
                (parts[0], parts[1])
            };

            let proposed_next_actions: Vec<String> = tail
                .split(ACTION_BREAK)
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
        })();

        match res {
            Ok(res) => Ok(res),
            Err(e) => {
                warn!("Incomplete output:\n{e:?}");
                Ok(TurnOutput {
                    image_description: image_description.trim().into(),
                    image_caption: image_caption.trim().into(),
                    text: output.trim().to_string(),
                    secret_info: tail.trim().to_string(),
                    proposed_next_actions: ["Missing".into(), "Missing".into(), "Missing".into()],
                    input_tokens: value.input_tokens,
                    output_tokens: value.output_tokens,
                })
            }
        }
    }
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
}
