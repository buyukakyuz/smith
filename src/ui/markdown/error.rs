use thiserror::Error;

#[derive(Debug, Error)]
pub enum MarkdownError {
    #[error("failed to parse markdown: {0}")]
    Parse(String),
}
