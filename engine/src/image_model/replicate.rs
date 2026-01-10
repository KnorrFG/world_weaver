use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::time::sleep;

use crate::{ImageModel, image_model::ProvidedModel};

use super::Image;

#[derive(Clone)]
pub struct ReplicateImageModel {
    url: String,
    model: ProvidedModel,
    client: Client,
    api_key: String,
    version: Option<String>,
    input_builder: Arc<dyn Fn(&str) -> serde_json::Value + Send + Sync>,
}

impl ReplicateImageModel {
    pub fn new(
        url: String,
        model: ProvidedModel,
        api_key: String,
        version: Option<String>,
        input_builder: impl Fn(&str) -> serde_json::Value + Send + Sync + 'static,
    ) -> Self {
        Self {
            url,
            model,
            client: Client::new(),
            api_key,
            version,
            input_builder: Arc::new(input_builder),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PredictionResponse {
    status: String,
    output: Option<serde_json::Value>,
}

impl ImageModel for ReplicateImageModel {
    fn get_image<'a>(
        &'a self,
        description: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Image>> + Send + 'a>> {
        Box::pin(async move {
            // 1. Create prediction
            let req_body = if let Some(v) = &self.version {
                json!({
                    "version": v,
                    "input": (self.input_builder)(description),
                })
            } else {
                json!({
                    "input": (self.input_builder)(description),
                })
            };
            let create_resp = self
                .client
                .post(&self.url)
                .bearer_auth(&self.api_key)
                .json(&req_body)
                .send()
                .await?;

            let status = create_resp.status();
            let body = create_resp.text().await?;
            ensure!(
                status.is_success(),
                "Prediciton Request error: {status} - {body}"
            );

            let prediction_infos = serde_json::from_str::<serde_json::Value>(&body)?;

            let prediction_url = prediction_infos["urls"]["get"]
                .as_str()
                .ok_or_else(|| eyre!("Missing prediction get URL:\n{prediction_infos:#?}"))?
                .to_string();

            // 2. Poll until finished
            loop {
                let resp = self
                    .client
                    .get(&prediction_url)
                    .bearer_auth(&self.api_key)
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<PredictionResponse>()
                    .await?;

                match resp.status.as_str() {
                    "succeeded" => {
                        let url = extract_image_url(
                            resp.output.as_ref().ok_or(eyre!("No output image"))?,
                        )?;
                        // 3. Download image
                        let bytes = self
                            .client
                            .get(url)
                            .send()
                            .await?
                            .error_for_status()?
                            .bytes()
                            .await?;

                        return Ok(Image {
                            data: bytes.to_vec(),
                            cost: None,
                        });
                    }
                    "failed" | "canceled" => {
                        return Err(eyre!("Replicate prediction failed:\n{resp:#?}"));
                    }
                    _ => {
                        sleep(Duration::from_millis(500)).await;
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

fn extract_image_url(output: &serde_json::Value) -> Result<&str> {
    match output {
        serde_json::Value::String(url) => Ok(url),

        serde_json::Value::Array(arr) => Ok(arr
            .first()
            .ok_or_else(|| eyre!("Empty output array"))?
            .as_str()
            .ok_or(eyre!("unexpected json"))?),

        other => Err(eyre!("Unsupported output format: {other:#?}")),
    }
}
