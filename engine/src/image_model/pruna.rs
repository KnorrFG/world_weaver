use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use reqwest::Client;
use serde::Deserialize;
use tokio::time::sleep;

use crate::{ImageModel, image_model::ProvidedModel};

use super::Image;

#[derive(Clone)]
pub struct PrunaImageModel {
    url: String,
    model: ProvidedModel,
    model_id: String,
    client: Client,
    api_key: String,
    input_builder: Arc<dyn Fn(&str) -> serde_json::Value + Send + Sync>,
}

impl PrunaImageModel {
    pub fn new(
        url: String,
        model: ProvidedModel,
        model_id: String,
        api_key: String,
        input_builder: impl Fn(&str) -> serde_json::Value + Send + Sync + 'static,
    ) -> Self {
        Self {
            url,
            model,
            model_id,
            client: Client::new(),
            api_key,
            input_builder: Arc::new(input_builder),
        }
    }
}

#[derive(Debug, Deserialize)]
struct AsyncPredictionResponse {
    get_url: String,
}

#[derive(Debug, Deserialize)]
struct SyncPredictionResponse {
    status: String,
    generation_url: Option<String>,
    error: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PredictionStatusResponse {
    status: String,
    generation_url: Option<String>,
    error: Option<String>,
    message: Option<String>,
}

impl ImageModel for PrunaImageModel {
    fn get_image<'a>(
        &'a self,
        description: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Image>> + Send + 'a>> {
        Box::pin(async move {
            let req_body = serde_json::json!({
                "input": (self.input_builder)(description),
            });

            let create_resp = self
                .client
                .post(&self.url)
                .header("apikey", &self.api_key)
                .header("Model", &self.model_id)
                .header("Try-Sync", "true")
                .json(&req_body)
                .send()
                .await?;

            let status = create_resp.status();
            let body = create_resp.text().await?;
            ensure!(
                status.is_success(),
                "Pruna prediction request error: {status} - {body}"
            );

            if let Ok(sync_resp) = serde_json::from_str::<SyncPredictionResponse>(&body)
            {
                match sync_resp.status.as_str() {
                    "succeeded" => {
                        let url = sync_resp
                            .generation_url
                            .ok_or_else(|| eyre!("Pruna sync response missing generation_url"))?;
                        let data = fetch_image_bytes(&self.client, &self.api_key, &url).await?;
                        return Ok(Image { data, cost: None });
                    }
                    "failed" | "canceled" => {
                        return Err(eyre!(
                            "Pruna prediction {}: {}{}",
                            sync_resp.status,
                            sync_resp.message.unwrap_or_default(),
                            sync_resp
                                .error
                                .map(|e| format!("\n{e}"))
                                .unwrap_or_default()
                        ));
                    }
                    _ => {}
                }
            }

            let prediction = serde_json::from_str::<AsyncPredictionResponse>(&body)?;

            loop {
                let resp = self
                    .client
                    .get(&prediction.get_url)
                    .header("apikey", &self.api_key)
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<PredictionStatusResponse>()
                    .await?;

                match resp.status.as_str() {
                    "succeeded" => {
                        let url = resp
                            .generation_url
                            .ok_or_else(|| eyre!("Pruna prediction succeeded without generation_url"))?;
                        let data = fetch_image_bytes(&self.client, &self.api_key, &url).await?;
                        return Ok(Image { data, cost: None });
                    }
                    "failed" | "canceled" => {
                        return Err(eyre!(
                            "Pruna prediction {}: {}{}",
                            resp.status,
                            resp.message.unwrap_or_default(),
                            resp.error
                                .map(|e| format!("\n{e}"))
                                .unwrap_or_default()
                        ));
                    }
                    "starting" | "processing" => sleep(Duration::from_millis(500)).await,
                    other => {
                        return Err(eyre!("Unexpected Pruna prediction status: {other}"));
                    }
                }
            }
        })
    }

    fn clone(&self) -> Box<dyn ImageModel + Send + 'static> {
        Box::new(Clone::clone(self))
    }

    fn provided_model(&self) -> ProvidedModel {
        self.model
    }
}

async fn fetch_image_bytes(client: &Client, api_key: &str, url: &str) -> Result<Vec<u8>> {
    let url = if url.starts_with("http://") || url.starts_with("https://") {
        url.to_string()
    } else {
        format!("https://api.pruna.ai{url}")
    };

    Ok(client
        .get(url)
        .header("apikey", api_key)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?
        .to_vec())
}
