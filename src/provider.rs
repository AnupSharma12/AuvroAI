pub trait Provider {
    fn name(&self) -> &str;

    fn generate_reply(&self, prompt: &str, conversation: &[String]) -> Result<String, String>;
}

pub struct MockProvider;

impl Provider for MockProvider {
    fn name(&self) -> &str {
        "Mock Provider"
    }

    fn generate_reply(&self, prompt: &str, conversation: &[String]) -> Result<String, String> {
        let previous_messages = conversation.len();
        Ok(format!(
            "Streaming demo response from {}: I received '{}' with {} prior messages and this output is rendered token-by-token so the chat feels live.",
            self.name(),
            prompt,
            previous_messages
        ))
    }
}
