use std::pin::Pin;

use color_eyre::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use strum::{Display, EnumIter};

pub mod flux2;
pub use flux2::Flux2;

pub mod replicate;

use crate::ImgModBox;

#[derive(
    Debug,
    Clone,
    Copy,
    Display,
    clap::ValueEnum,
    Serialize,
    Deserialize,
    Hash,
    PartialEq,
    Eq,
    EnumIter,
    Default,
)]
pub enum Model {
    Flux1,
    #[default]
    Flux2,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Display,
    clap::ValueEnum,
    Serialize,
    Deserialize,
    Hash,
    PartialEq,
    Eq,
    EnumIter,
)]
pub enum ModelProvider {
    #[strum(to_string = "Black Forest Labs")]
    BFL, // BlackForestLabs
    Replicate,
}

impl Model {
    pub fn make(&self, key: String) -> ImgModBox {
        match self {
            Model::Flux1 => Box::new(replicate::ReplicateImageModel::new(
                *self,
                key,
                "8cf067a09fbd627c5597781951e1a6988e3b69f6ef712b4948d3d2b5361569ad".into(),
                |prompt| {
                    json!({
                        "prompt": prompt,
                        "width": 832,
                        "height": 1216,
                        "steps": 25,
                        "cfg_scale": 3,
                        "seed": -1,
                    })
                },
            )),
            Model::Flux2 => Box::new(Flux2::new(key)),
        }
    }

    pub fn extra_generation_instructions(&self) -> &'static str {
        match self {
            Model::Flux1 => {
                r"
                    The model is Flux1. Keep the input short, everything after 77 tokens will be clipped.
                "
            }
            Model::Flux2 => {
                r"
                   The model is Flux2. Make sure to formulate the image description in a way that avoids
                   image moderation because of sexual content. When you describe anatomy,
                   do it in a non-erotic way.
                "
            }
        }
    }

    pub fn provider(&self) -> ModelProvider {
        match self {
            Model::Flux1 => ModelProvider::Replicate,
            Model::Flux2 => ModelProvider::BFL,
        }
    }
}

pub struct Image {
    pub data: Vec<u8>,
    pub cost: Option<f64>,
}

pub trait ImageModel {
    fn get_image<'a>(
        &'a self,
        description: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Image>> + Send + 'a>>;

    fn clone(&self) -> Box<dyn ImageModel + Send + 'static>;
    fn model(&self) -> Model;
}
