use crate::{image_model::ImageModel, llm::LLM};

pub type LLMBox = Box<dyn LLM + Send>;
pub type ImgModBox = Box<dyn ImageModel + Send>;
pub const N_PROPOSED_OPTIONS: usize = 3;
pub const HIST_SIZE: usize = 8;

pub mod game;
pub mod image_model;
pub mod llm;
pub mod save_archive;
