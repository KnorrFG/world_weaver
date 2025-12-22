#![allow(dead_code)]
/// this module holds types related to API responses. I don't want warnings
/// just cause the fields are unused
use bytes::Bytes;
use color_eyre::{Result, eyre::bail};
use serde::Deserialize;

use super::ClaudeApiError;

#[derive(Debug)]
pub struct RawEvent {
    /// Value from `event:` (e.g. "content_block_delta", "ping")
    pub event_type: Option<String>,

    /// Concatenated `data:` payload (may contain newlines)
    pub data: String,
}

#[derive(Debug)]
pub enum Event {
    MessageStart(MessageStart),
    ContentBlockStart(ContentBlockStart),
    ContentBlockDelta(ContentBlockDelta),
    ContentBlockStop(ContentBlockStop),
    MessageDelta(MessageDelta),
    MessageStop,
    Ping,
    Error(super::ClaudeApiError),
    Unknown(RawEvent),
}

#[derive(Debug, Deserialize)]
pub struct MessageStart {
    pub message: MessageInfo,
}

#[derive(Debug, Deserialize)]
pub struct MessageInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub role: String,
    // will always be empty according to doc
    pub content: Vec<serde_json::Value>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct ContentBlockStart {
    pub index: usize,
    pub content_block: ContentBlock,
}

#[derive(Debug, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ContentBlockDelta {
    pub index: usize,
    pub delta: TextDelta,
}

#[derive(Debug, Deserialize)]
pub struct TextDelta {
    #[serde(rename = "type")]
    pub delta_type: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ContentBlockStop {
    pub index: usize,
}

#[derive(Debug, Deserialize)]
pub struct MessageDelta {
    pub delta: MessageDeltaInner,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct MessageDeltaInner {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Usage {
    pub input_tokens: Option<usize>,
    pub output_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct ErrorEvent {
    pub error: InnerError,
}

#[derive(Debug, Deserialize)]
pub struct InnerError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[derive(Default)]
pub struct Parser {
    bytes: Vec<u8>,
    index: usize,
}

impl Parser {
    /// Feed a chunk into the parser, returning all complete events
    pub fn process(&mut self, chunk: Bytes) -> Result<Vec<Event>> {
        self.bytes.extend_from_slice(&chunk);
        let mut events = vec![];

        loop {
            // search for \n\n in unprocessed buffer
            let unprocessed = &self.bytes[self.index..];
            if let Some(pos) = unprocessed.windows(2).position(|w| w == b"\n\n") {
                let event_bytes = &unprocessed[..pos];
                let event = Self::parse_sse_event(event_bytes)?;
                events.push(Event::from_raw_event(event));
                self.index += pos + 2; // advance past the \n\n
            } else {
                break;
            }
        }

        Ok(events)
    }

    /// Parse any remaining bytes in the buffer as a final event (even without \n\n)
    pub fn parse_remaining(&mut self) -> Option<Event> {
        if self.index < self.bytes.len() {
            let remaining = &self.bytes[self.index..];
            let event = Self::parse_sse_event(remaining).ok()?;
            self.index = self.bytes.len();
            Some(Event::from_raw_event(event))
        } else {
            None
        }
    }

    /// Parses a single raw SSE event from the buffer.
    /// Returns (RawEvent, number of bytes consumed) if successful.
    fn parse_sse_event(buf: &[u8]) -> Result<RawEvent> {
        let text = std::str::from_utf8(buf)?;
        let mut event_type = None;
        let mut data = String::new();

        for line in text.lines() {
            if let Some(prefix) = line.strip_prefix("event: ") {
                event_type = Some(prefix.to_string());
            } else if let Some(prefix) = line.strip_prefix("data: ") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(prefix);
            } else {
                bail!("Unexpected line while parsing SSE event {line}\n\nEvent:\n{text}");
            }
        }

        Ok(RawEvent { event_type, data })
    }
}

impl Event {
    pub fn from_raw_event(raw: RawEvent) -> Self {
        (|| -> Option<Event> {
            match raw.event_type.as_deref()? {
                "message_start" => Some(Event::MessageStart(serde_json::from_str(&raw.data).ok()?)),
                "content_block_start" => Some(Event::ContentBlockStart(
                    serde_json::from_str(&raw.data).ok()?,
                )),
                "content_block_delta" => Some(Event::ContentBlockDelta(
                    serde_json::from_str(&raw.data).ok()?,
                )),
                "content_block_stop" => Some(Event::ContentBlockStop(
                    serde_json::from_str(&raw.data).ok()?,
                )),
                "message_delta" => Some(Event::MessageDelta(serde_json::from_str(&raw.data).ok()?)),
                "message_stop" => Some(Event::MessageStop),
                "ping" => Some(Event::Ping),
                "error" => {
                    // Deserialize the inner error
                    let err_event: ErrorEvent = serde_json::from_str(&raw.data).ok()?;
                    // Convert the string type into ClaudeApiError
                    Some(Event::Error(ClaudeApiError::from_type(
                        &err_event.error.error_type,
                        &err_event.error.message,
                    )))
                }
                _ => None,
            }
        })()
        .unwrap_or(Event::Unknown(raw))
    }
}

#[cfg(test)]
mod test {
    use bytes::Bytes;

    use super::*;

    #[test]
    fn test_parser_streaming() {
        let mut parser = Parser::default();

        // Concatenate multiple SSE events into a single "stream"
        let sse_data = b"event: message_start
data: {\"type\": \"message_start\", \"message\": {\"id\": \"msg_1\", \"type\": \"message\", \"role\": \"assistant\", \"content\": [], \"model\": \"claude-sonnet-4-5\", \"stop_reason\": null, \"stop_sequence\": null, \"usage\": {\"input_tokens\": 25, \"output_tokens\": 1}}}

event: content_block_start
data: {\"type\": \"content_block_start\", \"index\": 0, \"content_block\": {\"type\": \"text\", \"text\": \"\"}}

event: content_block_delta
data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \"Hello\"}}

event: content_block_delta
data: {\"type\": \"content_block_delta\", \"index\": 0, \"delta\": {\"type\": \"text_delta\", \"text\": \"!\"}}

event: content_block_stop
data: {\"type\": \"content_block_stop\", \"index\": 0}

event: message_delta
data: {\"type\": \"message_delta\", \"delta\": {\"stop_reason\": \"end_turn\", \"stop_sequence\":null}, \"usage\": {\"output_tokens\": 15}}

event: message_stop
data: {\"type\": \"message_stop\"}

event: ping
data: {\"type\": \"ping\"}

event: error
data: {\"type\": \"error\", \"error\": {\"type\": \"overloaded_error\", \"message\": \"Overloaded\"}}
";

        // Split the "stream" into arbitrary chunks to simulate streaming behavior
        let mut chunks: Vec<Bytes> = vec![];
        for slice in sse_data.chunks(50) {
            chunks.push(Bytes::from(slice.to_vec()));
        }

        let mut events = vec![];

        // Feed each chunk into the parser
        for chunk in chunks {
            let mut new_events = parser.process(chunk).unwrap();
            events.append(&mut new_events);
        }

        events.push(parser.parse_remaining().unwrap());

        assert_eq!(events.len(), 9);

        // Check types of each event
        assert!(matches!(events[0], Event::MessageStart(_)));
        assert!(matches!(events[1], Event::ContentBlockStart(_)));
        assert!(matches!(events[2], Event::ContentBlockDelta(_)));
        assert!(matches!(events[3], Event::ContentBlockDelta(_)));
        assert!(matches!(events[4], Event::ContentBlockStop(_)));
        assert!(matches!(events[5], Event::MessageDelta(_)));
        assert!(matches!(events[6], Event::MessageStop));
        assert!(matches!(events[7], Event::Ping));
        assert!(matches!(events[8], Event::Error(_)));

        // Verify content of ContentBlockDelta
        if let Event::ContentBlockDelta(delta) = &events[2] {
            assert_eq!(delta.delta.text, "Hello");
        }
        if let Event::ContentBlockDelta(delta) = &events[3] {
            assert_eq!(delta.delta.text, "!");
        }

        // Verify error mapping
        assert!(matches!(
            events[8],
            Event::Error(ClaudeApiError::Overloaded { .. })
        ));
    }
}
