#[derive(Debug, Clone)]
pub struct LoopConfig {
    pub max_iterations: usize,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            max_tokens: 4096,
            temperature: 1.0,
        }
    }
}
