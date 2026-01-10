use std::io::{Write, stdout};

use clap::Parser;
use color_eyre::Result;
use engine::{
    llm::ProvidedModel,
    llm::{InputMessage, Request, ResponseFragment, Role},
};
use tokio::pin;
use tokio_stream::StreamExt;

#[derive(clap::Parser)]
pub struct Cli {
    api_key: String,
    model: ProvidedModel,
    max_tokens: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    pretty_env_logger::init();
    color_eyre::install().unwrap();

    let mut model = args.model.make(args.api_key);
    let stream = model.send_request_stream(Request {
        messages: vec![InputMessage {
            role: Role::User,
            content: "Explain Rust futures by going way too deep".into(),
        }],
        max_tokens: args.max_tokens,
        system: None,
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
