use super::types::Message;

#[derive(Debug, Clone, Default)]
pub struct Memory {
    system_prompt: Option<String>,
    messages: Vec<Message>,
}

impl Memory {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            system_prompt: None,
            messages: Vec::new(),
        }
    }

    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = Some(prompt.into());
    }

    #[must_use]
    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    #[must_use]
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }
}
