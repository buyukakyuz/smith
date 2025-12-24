pub mod error;
pub mod factory;
pub mod http;
pub mod types;

pub mod anthropic;
pub mod gemini;
pub mod mock;
pub mod openai;
pub mod openai_compat;

pub use factory::create_provider;
pub use types::ApiKey;

pub use anthropic::AnthropicProvider;
pub use gemini::GeminiProvider;
pub use openai::OpenAIProvider;
pub use openai_compat::OpenAICompatProvider;

pub type OpenAIClient = OpenAIProvider;
