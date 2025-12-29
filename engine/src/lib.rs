use serde_json::Value;

use crate::{image_model::ImageModel, llm::LLM};

pub type LLMBox = Box<dyn LLM + Send>;
pub type ImgModBox = Box<dyn ImageModel + Send>;
pub const N_PROPOSED_OPTIONS: usize = 3;
pub const HIST_SIZE: usize = 8;

pub mod game;
pub mod image_model;
pub mod llm;
pub mod save_archive;

fn get_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        match current {
            Value::Object(map) => {
                current = map.get(*key)?;
            }
            _ => return None,
        }
    }
    Some(current)
}
