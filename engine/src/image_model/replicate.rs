use std::{future::Future, pin::Pin, sync::Arc, time::Duration};

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::time::sleep;

use crate::{ImageModel, image_model::Model};

use super::Image;

#[derive(Clone)]
pub struct ReplicateImageModel {
    model: Model,
    client: Client,
    api_key: String,
    version: String,
    input_builder: Arc<dyn Fn(&str) -> serde_json::Value + Send + Sync>,
}

impl ReplicateImageModel {
    pub fn new(
        model: Model,
        api_key: String,
        version: String,
        input_builder: impl Fn(&str) -> serde_json::Value + Send + Sync + 'static,
    ) -> Self {
        Self {
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
    output: Option<Vec<String>>,
}

impl ImageModel for ReplicateImageModel {
    fn get_image<'a>(
        &'a self,
        description: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Image>> + Send + 'a>> {
        Box::pin(async move {
            // 1. Create prediction
            let create_resp = self
                .client
                .post("https://api.replicate.com/v1/predictions")
                .bearer_auth(&self.api_key)
                .json(&json!({
                    "version": self.version,
                    "input": (self.input_builder)(description),
                }))
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
                        let url = resp
                            .output
                            .and_then(|o| o.into_iter().next())
                            .ok_or_else(|| eyre!("No output image"))?;

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

    fn model(&self) -> Model {
        self.model
    }
}
