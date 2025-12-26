use color_eyre::{Result, eyre::eyre};
use engine::image_model::flux2::flux2_api::{poll_and_fetch, query};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let api_key = std::env::args()
        .nth(1)
        .ok_or(eyre!("Missing api key as first arg"))?;
    let client = reqwest::Client::new();
    let query_resp = query("A futuristic city at sunset", &api_key, &client).await?;
    println!("Polling URL: {query_resp:#?}");

    let image_bytes = poll_and_fetch(&query_resp.polling_url, &api_key, &client).await?;
    std::fs::write("output.jpeg", &image_bytes)?;
    println!("Saved image, {} bytes", image_bytes.len());

    Ok(())
}
