use std::{fmt::Display, pin::Pin};

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
    clap::ValueEnum,
    Serialize,
    Deserialize,
    Hash,
    PartialEq,
    Eq,
    EnumIter,
    Default,
)]

pub enum ProvidedModel {
    Flux1Replicate,
    Flux2BLF,
    #[default]
    Flux2Replicate,
}

impl Display for ProvidedModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.model(), self.provider())
    }
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
    PartialOrd,
    Ord,
)]
pub enum Model {
    Flux1,
    Flux2,
}

impl Model {
    pub fn extra_generation_instructions(&self) -> &'static str {
        match self {
            Self::Flux1 => {
                r"
                    The image model is Flux1. Keep the input short, everything after 77 tokens will be clipped.
                "
            }
            Self::Flux2 => {
                r"
                   The image model is Flux2. Make sure to formulate the image description in a way that avoids
                   image moderation because of sexual content. When you describe anatomy,
                   do it in a non-erotic way.
                "
            }
        }
    }
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
    PartialOrd,
    Ord,
)]
pub enum ModelProvider {
    #[strum(to_string = "Black Forest Labs")]
    BFL,
    Replicate,
}

impl ProvidedModel {
    pub fn make(&self, key: String) -> ImgModBox {
        match self {
            ProvidedModel::Flux1Replicate => Box::new(replicate::ReplicateImageModel::new(
                "https://api.replicate.com/v1/predictions".into(),
                *self,
                key,
                Some("8cf067a09fbd627c5597781951e1a6988e3b69f6ef712b4948d3d2b5361569ad".into()),
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
            ProvidedModel::Flux2BLF => Box::new(Flux2::new(key)),
            ProvidedModel::Flux2Replicate => Box::new(replicate::ReplicateImageModel::new(
                "https://api.replicate.com/v1/models/black-forest-labs/flux-2-pro/predictions"
                    .into(),
                *self,
                key,
                None,
                |prompt| {
                    json!({
                        "width": 832,
                        "height": 1216,
                        "prompt": prompt,
                        "resolution": "1 MP",
                        "aspect_ratio": "9:16",
                        "input_images": [],
                        "output_format": "jpg",
                        "output_quality": 80,
                        "safety_tolerance": 5
                    })
                },
            )),
        }
    }

    pub fn provider(&self) -> ModelProvider {
        match self {
            ProvidedModel::Flux1Replicate => ModelProvider::Replicate,
            ProvidedModel::Flux2Replicate => ModelProvider::Replicate,
            ProvidedModel::Flux2BLF => ModelProvider::BFL,
        }
    }

    pub fn model(&self) -> Model {
        match self {
            ProvidedModel::Flux1Replicate => Model::Flux1,
            ProvidedModel::Flux2BLF => Model::Flux2,
            ProvidedModel::Flux2Replicate => Model::Flux2,
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
    fn provided_model(&self) -> ProvidedModel;
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelStyle {
    pub prefix: String,
    pub postfix: String,
}
