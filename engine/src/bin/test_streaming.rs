use std::io::{Write, stdout};

use color_eyre::{Result, eyre::eyre};
use engine::llm::{Claude, InputMessage, LLM, Request, ResponseFragment, Role};
use tokio::pin;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    color_eyre::install().unwrap();

    let token = std::env::args()
        .nth(1)
        .ok_or(eyre!("Need token as first cli arg"))?;

    let mut claude = Claude::new(token, "claude-sonnet-4-5".into());
    let stream = claude.send_request_stream(Request {
        messages: vec![InputMessage {
            role: Role::User,
            content: "Explain Rust futures by going way too deep".into(),
        }],
        max_tokens: 300,
    });

    pin!(stream);
    while let Some(fragment) = stream.try_next().await? {
        match fragment {
            ResponseFragment::TextDelta(t) => {
                print!("{t}");
                stdout().flush()?;
            }
            ResponseFragment::MessageComplete(output_message) => {
                println!(
                    "Cost: input: {}, output: {}",
                    output_message.input_tokens, output_message.output_tokens
                );
            }
        }
    }
    Ok(())
}
