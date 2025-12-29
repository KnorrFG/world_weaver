use clap::Parser;
use color_eyre::{Result, eyre::eyre};
use engine::image_model::{
    Model,
    flux2::flux2_api::{poll_and_fetch, query},
};

#[derive(clap::Parser)]
struct Arg {
    model: Model,
    key: String,
    description: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let Arg {
        model,
        key,
        description,
    } = Arg::parse();
    let llm = model.make(key);

    let image = llm.get_image(&description).await?;
    std::fs::write("output.jpeg", &image.data)?;
    println!("Saved image, {} bytes", image.data.len());

    Ok(())
}
