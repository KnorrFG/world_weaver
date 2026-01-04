use std::pin::Pin;

use color_eyre::{Result, eyre::Context};
use log::debug;

use crate::image_model::{Image, ImageModel};

use super::ProvidedModel;

pub mod flux2_api;

#[derive(Clone)]
pub struct Flux2 {
    api_key: String,
    client: reqwest::Client,
}

impl Flux2 {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

impl ImageModel for Flux2 {
    fn get_image<'a>(
        &'a self,
        description: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Image>> + Send + 'a>> {
        let resp_fut = flux2_api::query(description, &self.api_key, &self.client);

        Box::pin(async move {
            let response = resp_fut.await?;
            let cost = response.cost;
            debug!("Query response: {response:#?}");
            let data =
                flux2_api::poll_and_fetch(&response.polling_url, &self.api_key, &self.client)
                    .await
                    .with_context(|| format!("Image description:\n{description}"))?;
            Ok(Image {
                data,
                cost: Some(cost),
            })
        })
    }

    fn clone(&self) -> Box<dyn ImageModel + Send + 'static> {
        Box::new(Clone::clone(self))
    }

    fn provided_model(&self) -> ProvidedModel {
        ProvidedModel::Flux2BLF
    }
}
