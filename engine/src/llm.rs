use std::pin::Pin;

use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter};
use tokio_stream::Stream;

use color_eyre::Result;

pub trait LLM {
    fn send_request_stream(&mut self, req: Request) -> LLMStream<'_>;
    fn clone(&self) -> Box<dyn LLM + Send + 'static>;
}

pub type LLMStream<'a> = Pin<Box<dyn Stream<Item = Result<ResponseFragment>> + Send + 'a>>;

#[derive(Debug)]
pub enum ResponseFragment {
    TextDelta(String),
    MessageComplete(OutputMessage),
}

pub struct Request {
    pub system: Option<String>,
    pub messages: Vec<InputMessage>,
    pub max_tokens: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InputMessage {
    pub role: Role,
    pub content: String,
}

impl InputMessage {
    pub(crate) fn user(user_message: String) -> InputMessage {
        Self {
            role: Role::User,
            content: user_message,
        }
    }

    pub(crate) fn assistant(assistant_message: String) -> InputMessage {
        Self {
            role: Role::Assistant,
            content: assistant_message,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutputMessage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, EnumIter, Display,
)]
pub enum ModelProvider {
    Anthropic,
    Openrouter,
}

#[derive(
    Debug,
    Clone,
    Copy,
    clap::ValueEnum,
    Serialize,
    Deserialize,
    Default,
    EnumIter,
    Display,
    PartialEq,
    Eq,
)]
pub enum ProvidedModel {
    #[strum(to_string = "Claude Sonette - latest (Anthropic)")]
    ClaudeSonette,

    #[strum(to_string = "Claude Sonette 4.5 (Anthropic)")]
    ClaudeSonette45,

    #[strum(to_string = "Claude Haiku - latest (Anthropic)")]
    ClaudeHaiku,

    #[strum(to_string = "Aion-2.0 (openrouter.ai)")]
    Aion2Openr,

    #[strum(to_string = "Kimmi K2.5 (openrouter.ai)")]
    Flex,

    #[default]
    #[strum(to_string = "GLM 5 (openrouter.ai)")]
    Glm5,
}

impl ProvidedModel {
    pub fn make(self, api_key: String) -> LLMBox {
        match self {
            ProvidedModel::ClaudeSonette => {
                Box::new(Claude::new(api_key, "claude-sonnet-4-6".into()))
            }
            ProvidedModel::ClaudeSonette45 => {
                Box::new(Claude::new(api_key, "claude-sonnet-4-5".into()))
            }
            ProvidedModel::ClaudeHaiku => Box::new(Claude::new(api_key, "claude-haiku-4-5".into())),
            ProvidedModel::Aion2Openr => Box::new(OpenAIChat::new(
                api_key,
                "https://openrouter.ai/api/v1/chat/completions",
                "aion-labs/aion-2.0",
            )),
            ProvidedModel::Flex => Box::new(OpenAIChat::new(
                api_key,
                "https://openrouter.ai/api/v1/chat/completions",
                "moonshotai/kimi-k2.5",
            )),
            ProvidedModel::Glm5 => Box::new(OpenAIChat::new(
                api_key,
                "https://openrouter.ai/api/v1/chat/completions",
                "z-ai/glm-5",
            )),
        }
    }

    pub fn provider(self) -> ModelProvider {
        match self {
            ProvidedModel::ClaudeSonette => ModelProvider::Anthropic,
            ProvidedModel::ClaudeSonette45 => ModelProvider::Anthropic,
            ProvidedModel::ClaudeHaiku => ModelProvider::Anthropic,
            ProvidedModel::Aion2Openr => ModelProvider::Openrouter,
            ProvidedModel::Flex => ModelProvider::Openrouter,
            ProvidedModel::Glm5 => ModelProvider::Openrouter,
        }
    }
}

mod claude;
pub use claude::Claude;

use crate::LLMBox;

mod open_ai_chat;
pub use open_ai_chat::OpenAIChat;
