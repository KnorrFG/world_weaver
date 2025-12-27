use std::pin::Pin;

use serde::{Deserialize, Serialize};
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

mod claude;
pub use claude::Claude;
