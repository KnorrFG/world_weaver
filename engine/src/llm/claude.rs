use crate::llm::LLMStream;

use super::{LLM, Request};

mod claude_api;

pub struct Claude {
    pub(crate) api_key: String,
    pub(crate) model: String,
    pub(crate) client: reqwest::Client,
}

impl Claude {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }
}

impl LLM for Claude {
    fn send_request_stream<'a>(&'a mut self, req: Request) -> LLMStream<'a> {
        let Request {
            messages,
            max_tokens,
        } = req;

        let claude_req = claude_api::Request {
            api_key: self.api_key.clone(),
            data: claude_api::RequestBody {
                model: self.model.clone(),
                messages,
                max_tokens,
                stream: true,
            },
        };

        Box::pin(claude_api::send_request_stream(claude_req, &self.client))
    }
}
