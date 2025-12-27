use std::pin::Pin;

use color_eyre::Result;

pub mod flux2;
pub use flux2::Flux2;

pub struct Image {
    pub data: Vec<u8>,
    pub cost: f64,
}

pub trait ImageModel {
    fn get_image<'a>(
        &'a self,
        description: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Image>> + Send + 'a>>;

    fn clone(&self) -> Box<dyn ImageModel + Send + 'static>;
}
