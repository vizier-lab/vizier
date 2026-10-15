//! Media and embedding clients. They implement vizier's existing `Vizier*Model` traits
//! directly, over the same transport as completions (contract S9).

pub mod cohere;
pub mod gemini;
pub mod huggingface;
pub mod hyperbolic;
pub mod ollama;
pub mod openai_shaped;

pub use openai_shaped::Endpoint;

#[cfg(test)]
pub(crate) mod golden_tests;
