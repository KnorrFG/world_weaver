use async_stream::try_stream;
use color_eyre::eyre::{Context, eyre};
use log::{debug, error};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;

use super::{LLM, LLMStream, OutputMessage, Request, ResponseFragment, Role};

#[derive(Debug, Clone)]
pub struct OpenAIChat {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl OpenAIChat {
    pub fn new(api_key: String, base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.into(),
            model: model.into(), // change as needed
        }
    }
}

impl LLM for OpenAIChat {
    fn send_request_stream(&mut self, req: Request) -> LLMStream<'_> {
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let url = self.base_url.clone();
        let model = self.model.clone();

        Box::pin(try_stream! {
            // Build messages
            let mut messages = Vec::new();

            if let Some(system) = req.system {
                messages.push(OpenAIMessage {
                    role: "system",
                    content: system,
                });
            }

            for msg in req.messages {
                messages.push(OpenAIMessage {
                    role: match msg.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                    },
                    content: msg.content,
                });
            }

            let body = OpenAIChatRequest {
                model,
                messages,
                // max_tokens: req.max_tokens,
                stream: true,
            };

            let res = client
                .post(&url)
                .bearer_auth(api_key)
                .json(&body)
                .send()
                .await.context("initial response")?;


             if !res.status().is_success() {
                let status = res.status();
                let body = res.text().await.unwrap_or_default();
                Err(eyre!("OpenAI error {}: {}", status, body))?;
            } else {
                debug!("Openai response:\n{res:#?}");
                let mut stream = res.bytes_stream();

                let mut full_text = String::new();
                let mut input_tokens = 0usize;
                let mut output_tokens = 0usize;

                while let Some(chunk) = stream.next().await {
                    let Ok(chunk) = chunk else {
                        error!("streaming error:\n{chunk:?}");
                        break;
                    };

                    let text = std::str::from_utf8(&chunk).context("chunk to utf-8")?;

                    for line in text.lines() {
                        let Some(data) = line.trim().strip_prefix("data: ") else {
                            continue;
                        };

                        if data == "[DONE]" {
                            yield ResponseFragment::MessageComplete(OutputMessage {
                                input_tokens,
                                output_tokens,
                                text: full_text.clone(),
                            });
                            return;
                        }

                        let event: OpenAIStreamChunk = serde_json::from_str(data).context("parsing stream chunk")?;

                        if let Some(choice) = event.choices.first() {
                            if let Some(content) = &choice.delta.content {
                                output_tokens += 1; // token estimate; provider may differ
                                full_text.push_str(content);
                                yield ResponseFragment::TextDelta(content.clone());
                            }
                        }

                        if let Some(usage) = event.usage {
                            input_tokens = usage.prompt_tokens;
                            output_tokens = usage.completion_tokens;
                        }
                    }
                }
            }
        })
    }

    fn clone(&self) -> Box<dyn LLM + Send + 'static> {
        Box::new(Self {
            client: self.client.clone(),
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
        })
    }
}

//
// ===== OpenAI wire types =====
//

#[derive(Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    // max_tokens: usize,
    stream: bool,
}

#[derive(Serialize)]
struct OpenAIMessage {
    role: &'static str,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIStreamChunk {
    choices: Vec<OpenAIStreamChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
}

#[derive(Deserialize, Debug)]
struct OpenAIStreamChoice {
    delta: OpenAIDelta,
}

#[derive(Deserialize, Debug)]
struct OpenAIDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAIUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}
