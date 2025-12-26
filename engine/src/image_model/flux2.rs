use std::pin::Pin;

use color_eyre::Result;
use log::debug;

use crate::image_model::ImageModel;

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
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        let resp_fut = flux2_api::query(description, &self.api_key, &self.client);

        Box::pin(async move {
            let response = resp_fut.await?;
            debug!("Query response: {response:#?}");
            flux2_api::poll_and_fetch(&response.polling_url, &self.api_key, &self.client).await
        })
    }

    fn clone(&self) -> Box<dyn ImageModel + Send + 'static> {
        Box::new(Clone::clone(self))
    }
}
