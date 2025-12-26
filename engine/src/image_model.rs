use std::pin::Pin;

use color_eyre::Result;

pub mod flux2;
pub use flux2::Flux2;

pub trait ImageModel {
    fn get_image<'a>(
        &'a self,
        description: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;

    fn clone(&self) -> Box<dyn ImageModel + Send + 'static>;
}
