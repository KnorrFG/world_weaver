use std::time::Duration;

use async_stream::try_stream;
use color_eyre::{
    Result,
    eyre::{bail, eyre},
};
use log::{debug, info};
use reqwest::header::{self, HeaderValue};
use serde::{Deserialize, Serialize};
use tokio_stream::Stream;

mod error;
pub use error::ClaudeApiError;

use crate::llm::{InputMessage, OutputMessage, ResponseFragment};

mod sse_parser;

#[derive(Debug)]
pub struct Request {
    pub api_key: String,
    pub data: RequestBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RequestBody {
    pub model: String,
    pub messages: Vec<InputMessage>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,

    pub max_tokens: usize,
    pub stream: bool,
}

pub fn send_request_stream(
    mut req: Request,
    client: &reqwest::Client,
) -> impl Stream<Item = Result<ResponseFragment>> {
    try_stream! {
        req.data.stream = true;
        let request =client
            .post("https://api.anthropic.com/v1/messages")
            .timeout(Duration::from_secs(60*3))
            .json(&req.data)
            .header("x-api-key", &req.api_key)
            .header("anthropic-version", HeaderValue::from_static("2023-06-01"))
            .header(header::ACCEPT, HeaderValue::from_static("text/event-stream"));

        debug!("request: {request:#?}");
        debug!("Json-data: {}", serde_json::to_string(&req.data).unwrap());
        let res = request
            .send()
            .await?;

         if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            Err(eyre!("Anthropic error {}: {}", status, body))?;
        } else {
            let stream = res.bytes_stream();

            let mut parser = sse_parser::Parser::default();
            let mut input_tokens = 0;
            let mut output_tokens = 0;
            let mut text = String::new();
            let mut first_msg_complete = false;

            let mut process_event = |ev| -> Result<Option<ResponseFragment>> {
                use sse_parser::Event::*;
                match ev {
                    MessageStart(msg_start) => {
                        if first_msg_complete {
                            Err(eyre!("Unexpected second message"))?;
                        }
                        if msg_start.message.role != "assistant" {
                            Err(eyre!("Unexpected role in received message:\n{msg_start:#?}"))?;
                        }

                        let usage = msg_start.message.usage;
                        input_tokens += usage.input_tokens.ok_or(eyre!("Msg didn't contain input tokens:\n{msg_start:#?}"))?;
                        output_tokens += usage.output_tokens.ok_or(eyre!("Msg didn't contain output tokens:\n{msg_start:#?}"))?;
                    }

                    ContentBlockStart(block) => {
                        if block.content_block.block_type != "text" {
                            Err(eyre!("unexpected block type: {}", block.content_block.block_type))?;
                        }

                        if !block.content_block.text.is_empty() {
                            text.push_str(&block.content_block.text);
                            return Ok(Some(ResponseFragment::TextDelta(block.content_block.text)))
                        }
                    }

                    ContentBlockDelta(delta) => {
                        if delta.delta.delta_type != "text_delta" {
                            Err(eyre!("unexpected delta type: {}", delta.delta.delta_type))?;
                        }

                        text.push_str(&delta.delta.text);
                        return Ok(Some(ResponseFragment::TextDelta(delta.delta.text)));
                    }

                    MessageDelta(delta) => {
                        output_tokens += delta.usage.output_tokens.ok_or(eyre!("MessageDelta missing output tokens"))?;
                    }

                    ContentBlockStop(_) | Ping=> {
                    }

                    MessageStop => {
                        first_msg_complete = true;
                        return Ok(Some(ResponseFragment::MessageComplete(OutputMessage { input_tokens, output_tokens, text: text.clone() })))
                    }

                    Error(err) => {
                        Err(err)?;
                    }

                    Unknown(raw_event) => {
                        info!("Unknown event:\n{raw_event:#?}");
                    }
                }

                Ok(None)
            };

            for await chunk in stream {
                for ev in parser.process(chunk?)? {
                    if let Some(fragment) = process_event(ev)? {
                        yield fragment;
                    }
                }
            }

            if let Some(ev) = parser.parse_remaining() {
                // somehow, try_stream puts us back to Rust 2021
                if let Some(fragment) = process_event(ev)?{
                    yield fragment;
                }
            }
        }

    }
}

#[cfg(test)]
mod test {
    use expect_test::expect;

    use crate::llm::Role;

    use super::*;

    #[test]
    fn request_serialization() {
        let body = RequestBody {
            model: "model".into(),
            system: None,
            messages: vec![
                InputMessage {
                    role: Role::User,
                    content: "Some user msg".into(),
                },
                InputMessage {
                    role: Role::Assistant,
                    content: "Some Assitant msg".into(),
                },
            ],
            max_tokens: 200,
            stream: false,
        };

        let expect = expect![[r#"{"model":"model","messages":[{"role":"user","content":"Some user msg"},{"role":"assistant","content":"Some Assitant msg"}],"max_tokens":200,"stream":false}"#]];
        expect.assert_eq(&serde_json::to_string(&body).unwrap());
    }
}
