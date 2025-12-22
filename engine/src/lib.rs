use crate::llm::LLM;

pub mod llm;

pub type LLMBox = Box<dyn LLM + Send>;
pub const N_PROPOSED_OPTIONS: usize = 3;
pub const HIST_SIZE: usize = 8;

pub mod game;
