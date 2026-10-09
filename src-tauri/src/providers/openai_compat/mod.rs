//! Summary LLM over the OpenAI Chat Completions protocol (DashScope, DeepSeek,
//! OpenAI, Moonshot, Zhipu, Ark, or any custom compatible endpoint).

pub mod client;
pub mod presets;

pub use client::{HttpChatClient, LlmConfig, SummaryGenerateInput, SummaryGenerator};
