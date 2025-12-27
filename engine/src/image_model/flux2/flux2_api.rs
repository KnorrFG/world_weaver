use color_eyre::{
    Result,
    eyre::{bail, ensure, eyre},
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Deserialize)]
pub struct StartResponse {
    pub id: String,
    pub polling_url: String,
    pub cost: f64,
    pub input_mp: f64,
    pub output_mp: f64,
}

#[derive(Debug, Deserialize)]
pub struct PollResponse {
    pub id: String,
    pub status: String,
    pub result: Option<PollResult>,
    pub progress: Option<Value>,
    pub details: Option<Value>,
    pub preview: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct PollResult {
    pub start_time: f64,
    pub prompt: String,
    pub seed: u64,
    pub sample: String, // URL to the generated image
}

/// Starts a FLUX.2 Pro text-to-image job and returns the StartResponse
pub async fn query(prompt: &str, api_key: &str, client: &reqwest::Client) -> Result<StartResponse> {
    let payload = serde_json::json!({
        "prompt": prompt,
        "model": "flux-2-pro",
        "width": 832,
        "height": 1216,
        "safety_tolerance": 5,
    });

    let resp = client
        .post("https://api.bfl.ai/v1/flux-2-pro")
        .header("accept", "application/json")
        .header("x-key", api_key)
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    ensure!(
        status.is_success(),
        "Failed to start Flux2 job: {} - {}",
        status,
        text
    );

    Ok(serde_json::from_str(&text)?)
}

/// Polls a FLUX.2 Pro job until it's ready, then fetches the resulting image bytes
pub async fn poll_and_fetch(
    polling_url: &str,
    api_key: &str,
    client: &reqwest::Client,
) -> Result<Vec<u8>> {
    loop {
        let resp = client
            .get(polling_url)
            .header("accept", "application/json")
            .header("x-key", api_key)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("Flux2 poll failed {}: {}", status, body);
        }

        let poll: PollResponse = resp.json().await?;

        match poll.status.as_str() {
            "Ready" => {
                let url = &poll
                    .result
                    .as_ref()
                    .ok_or(eyre!("Missing result field:\n{poll:#?}"))?
                    .sample;
                let resp = client.get(url).send().await?;
                let bytes = resp.bytes().await?.to_vec();
                return Ok(bytes);
            }
            "Request Moderated" => bail!("Request moderated"),
            "Error" => bail!("Flux2 job failed:\n{poll:#?}"),
            _ => sleep(Duration::from_secs(1)).await,
        }
    }
}
