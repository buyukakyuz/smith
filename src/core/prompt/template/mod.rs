mod claude;
mod gemini;
mod openai;

pub use claude::CLAUDE_TEMPLATE;
pub use gemini::GEMINI_TEMPLATE;
pub use openai::OPENAI_TEMPLATE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TemplateType {
    #[default]
    Claude,
    OpenAI,
    Gemini,
}

impl TemplateType {
    #[must_use]
    pub const fn template(self) -> &'static str {
        match self {
            Self::Claude => CLAUDE_TEMPLATE,
            Self::OpenAI => OPENAI_TEMPLATE,
            Self::Gemini => GEMINI_TEMPLATE,
        }
    }
}
