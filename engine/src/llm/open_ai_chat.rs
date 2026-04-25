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
    provider_order: Vec<String>,
}

impl OpenAIChat {
    pub fn new(api_key: String, base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self::new_with_provider_order(
            api_key,
            base_url,
            model,
            std::iter::empty::<String>(),
        )
    }

    pub fn new_with_provider_order<I, S>(
        api_key: String,
        base_url: impl Into<String>,
        model: impl Into<String>,
        provider_order: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            client: Client::new(),
            api_key,
            base_url: base_url.into(),
            model: model.into(),
            provider_order: provider_order.into_iter().map(Into::into).collect(),
        }
    }
}

impl LLM for OpenAIChat {
    fn send_request_stream(&mut self, req: Request) -> LLMStream<'_> {
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let url = self.base_url.clone();
        let model = self.model.clone();
        let provider_order = self.provider_order.clone();

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
                model: model.clone(),
                messages,
                // max_tokens: req.max_tokens,
                stream: true,
                provider: OpenRouterProvider::from_order(provider_order),
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
                let mut chunk_count = 0usize;
                let mut data_line_count = 0usize;
                let mut last_chunk_preview = None::<String>;
                let mut last_data_line = None::<String>;

                while let Some(chunk) = stream.next().await {
                    let chunk = match chunk {
                        Ok(chunk) => chunk,
                        Err(err) => {
                            error!("streaming error:\n{err:?}");
                            Err(err).context("stream chunk")?
                        }
                    };

                    chunk_count += 1;
                    let text = std::str::from_utf8(&chunk).context("chunk to utf-8")?;
                    last_chunk_preview = Some(text.chars().take(300).collect());

                    for line in text.lines() {
                        let Some(data) = line.trim().strip_prefix("data: ") else {
                            continue;
                        };
                        data_line_count += 1;
                        last_data_line = Some(data.chars().take(300).collect());

                        if data == "[DONE]" {
                            yield ResponseFragment::MessageComplete(OutputMessage {
                                input_tokens,
                                output_tokens,
                                text: full_text.clone(),
                            });
                            return;
                        }

                        let event: OpenAIStreamChunk = serde_json::from_str(data).context("parsing stream chunk")?;

                        if let Some(choice) = event.choices.first()
                            && let Some(content) = &choice.delta.content
                        {
                            output_tokens += 1; // token estimate; provider may differ
                            full_text.push_str(content);
                            yield ResponseFragment::TextDelta(content.clone());
                        }

                        if let Some(usage) = event.usage {
                            input_tokens = usage.prompt_tokens;
                            output_tokens = usage.completion_tokens;
                        }
                    }
                }

                error!(
                    "OpenAI stream ended without [DONE]. model={model}, url={url}, chunks={chunk_count}, data_lines={data_line_count}, text_len={}, input_tokens={}, output_tokens={}, last_chunk_preview={:?}, last_data_line={:?}",
                    full_text.len(),
                    input_tokens,
                    output_tokens,
                    last_chunk_preview,
                    last_data_line,
                );
                Err(eyre!("OpenAI stream ended without [DONE]"))?;
            }
        })
    }

    fn clone(&self) -> Box<dyn LLM + Send + 'static> {
        Box::new(Self {
            client: self.client.clone(),
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            provider_order: self.provider_order.clone(),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<OpenRouterProvider>,
}

#[derive(Serialize)]
struct OpenRouterProvider {
    order: Vec<String>,
}

impl OpenRouterProvider {
    fn from_order(order: Vec<String>) -> Option<Self> {
        if order.is_empty() {
            None
        } else {
            Some(Self { order })
        }
    }
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
