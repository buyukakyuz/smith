use super::env::EnvironmentInfo;
use super::template::TemplateType;
use crate::tools::ToolRegistry;

pub struct PromptBuilder {
    env_info: EnvironmentInfo,
    model_name: Option<String>,
    model_id: Option<String>,
    template_type: TemplateType,
    include_git_status: bool,
}

impl PromptBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self {
            env_info: EnvironmentInfo::collect(),
            model_name: None,
            model_id: None,
            template_type: TemplateType::default(),
            include_git_status: true,
        }
    }

    #[must_use]
    pub const fn with_template(mut self, template_type: TemplateType) -> Self {
        self.template_type = template_type;
        self
    }

    #[must_use]
    pub fn with_model(mut self, name: impl Into<String>, id: impl Into<String>) -> Self {
        self.model_name = Some(name.into());
        self.model_id = Some(id.into());
        self
    }

    #[must_use]
    pub fn build(&self, _tools: &ToolRegistry) -> String {
        let mut prompt = String::new();

        prompt.push_str(self.template_type.template());
        prompt.push_str("\n\n");

        prompt.push_str("Here is useful information about the environment you are running in:\n");
        prompt.push_str("<env>\n");
        prompt.push_str(&self.env_info.format());
        prompt.push_str("\n</env>\n");

        if let (Some(name), Some(id)) = (&self.model_name, &self.model_id) {
            match self.template_type {
                TemplateType::Claude => {
                    prompt.push_str(&format!(
                        "\nYou are powered by the model named {name}. The exact model ID is {id}.\n"
                    ));
                }
                TemplateType::OpenAI => {
                    prompt.push_str(&format!("\nModel: {name} ({id})\n"));
                }
                TemplateType::Gemini => {
                    prompt.push_str(&format!("\nModel: {name} ({id})\n"));
                }
            }
        }

        if self.include_git_status
            && let Some(git_status) = self.env_info.format_git_status()
        {
            prompt.push_str("\n\n");
            prompt.push_str(&git_status);
        }

        prompt
    }
}

impl Default for PromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_builder_basic_claude() {
        let builder = PromptBuilder::new();
        let tools = ToolRegistry::new();
        let prompt = builder.build(&tools);

        assert!(prompt.contains("You are an interactive CLI tool"));
        assert!(prompt.contains("# Tone and style"));

        assert!(prompt.contains("Working directory:"));
        assert!(prompt.contains("Platform:"));
        assert!(prompt.contains("Today's date:"));
    }

    #[test]
    fn test_prompt_builder_openai_template() {
        let builder = PromptBuilder::new().with_template(TemplateType::OpenAI);
        let tools = ToolRegistry::new();
        let prompt = builder.build(&tools);

        assert!(prompt.contains("You are ChatGPT"));
        assert!(prompt.contains("oververbosity"));

        assert!(prompt.contains("Working directory:"));
    }

    #[test]
    fn test_prompt_builder_with_model_claude() {
        let builder = PromptBuilder::new().with_model("Sonnet 4", "claude-sonnet-4-20250514");
        let tools = ToolRegistry::new();
        let prompt = builder.build(&tools);

        assert!(prompt.contains("Sonnet 4"));
        assert!(prompt.contains("claude-sonnet-4-20250514"));
    }

    #[test]
    fn test_prompt_builder_with_model_openai() {
        let builder = PromptBuilder::new()
            .with_template(TemplateType::OpenAI)
            .with_model("GPT-4", "gpt-4-turbo");
        let tools = ToolRegistry::new();
        let prompt = builder.build(&tools);

        assert!(prompt.contains("GPT-4"));
        assert!(prompt.contains("gpt-4-turbo"));
    }

    #[test]
    fn test_template_type_selection() {
        let claude_builder = PromptBuilder::new().with_template(TemplateType::Claude);
        let openai_builder = PromptBuilder::new().with_template(TemplateType::OpenAI);

        let tools = ToolRegistry::new();
        let claude_prompt = claude_builder.build(&tools);
        let openai_prompt = openai_builder.build(&tools);

        assert!(claude_prompt.contains("You are an interactive CLI tool"));
        assert!(!openai_prompt.contains("You are an interactive CLI tool"));

        assert!(!claude_prompt.contains("You are ChatGPT"));
        assert!(openai_prompt.contains("You are ChatGPT"));
    }
}
