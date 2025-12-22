pub const HELP_TEXT: &str = r"Available commands:
/help  - Show this help message
/clear - Clear the chat history
/model - Switch to a different model
/exit  - Exit the application";

pub const SLASH_COMMANDS: &[&str] = &["/help", "/exit", "/clear", "/model", "/save", "/load"];

pub enum SlashCommand {
    Help,
    Exit,
    Clear,
    Model,
    NotImplemented(String),
    Unknown(String),
}

impl SlashCommand {
    pub fn parse(input: &str) -> Self {
        let cmd = input.split_whitespace().next().unwrap_or("");
        match cmd {
            "/help" => Self::Help,
            "/exit" => Self::Exit,
            "/clear" => Self::Clear,
            "/model" => Self::Model,
            "/save" | "/load" => Self::NotImplemented(cmd.to_string()),
            _ => Self::Unknown(cmd.to_string()),
        }
    }
}
