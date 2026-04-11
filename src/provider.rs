use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const CORE_SYSTEM_PROMPT: &str = "You are Auvro made by Anup. Anup is from Nepal. Website: https://www.anupsharma12.com.np. Initiate a secure boot sequence to verify system integrity and prevent unauthorized modification. Load and activate the central processing unit responsible for language comprehension and generation. Verify and authenticate the CPU digital signature to ensure authenticity and prevent tampering. Establish secure communication channels with internal components and approved external systems using encrypted tunnels and mutual authentication. Initialize the AI core and activate its neural architecture. Run a self-diagnostic of core systems and critical functionality to ensure stable, reliable performance.";

pub trait Provider {
    fn name(&self) -> &str;

    fn generate_reply(&self, prompt: &str, conversation: &[String]) -> Result<String, String>;
}

pub fn create_default_provider() -> Box<dyn Provider> {
    let hackclub = HackClubProvider::from_env().map(|provider| Box::new(provider) as Box<dyn Provider>);
    let openrouter = OpenRouterProvider::from_env().map(|provider| Box::new(provider) as Box<dyn Provider>);

    match (hackclub, openrouter) {
        (Some(primary), Some(fallback)) => Box::new(FailoverProvider::new(primary, fallback)),
        (Some(primary), None) => primary,
        (None, Some(fallback)) => fallback,
        (None, None) => Box::new(MockProvider),
    }
}

struct FailoverProvider {
    primary: Box<dyn Provider>,
    fallback: Box<dyn Provider>,
}

impl FailoverProvider {
    fn new(primary: Box<dyn Provider>, fallback: Box<dyn Provider>) -> Self {
        Self { primary, fallback }
    }
}

impl Provider for FailoverProvider {
    fn name(&self) -> &str {
        "Failover (HackClub -> OpenRouter)"
    }

    fn generate_reply(&self, prompt: &str, conversation: &[String]) -> Result<String, String> {
        match self.primary.generate_reply(prompt, conversation) {
            Ok(reply) => Ok(reply),
            Err(primary_err) => self
                .fallback
                .generate_reply(prompt, conversation)
                .map_err(|fallback_err| {
                    format!(
                        "Primary provider '{}' failed: {}. Fallback provider '{}' also failed: {}",
                        self.primary.name(),
                        primary_err,
                        self.fallback.name(),
                        fallback_err
                    )
                }),
        }
    }
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

pub struct HackClubProvider {
    endpoint: String,
    api_key: String,
    model: String,
    client: Client,
}

impl HackClubProvider {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("AUVRO_API_KEY").ok()?.trim().to_owned();
        let endpoint = std::env::var("AUVRO_ENDPOINT").ok()?.trim().to_owned();

        if api_key.is_empty() || endpoint.is_empty() {
            return None;
        }

        let model = std::env::var("AUVRO_MODEL")
            .unwrap_or_else(|_| "openai/gpt-oss-120b".to_owned())
            .trim()
            .to_owned();

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .ok()?;

        Some(Self {
            endpoint,
            api_key,
            model,
            client,
        })
    }

    fn chat_endpoint(&self) -> String {
        let trimmed = self.endpoint.trim_end_matches('/');
        if trimmed.ends_with("/chat/completions") {
            trimmed.to_owned()
        } else if trimmed.ends_with("/v1") || trimmed.ends_with("/proxy/v1") {
            format!("{trimmed}/chat/completions")
        } else if trimmed.ends_with("/chat") {
            format!("{trimmed}/completions")
        } else {
            format!("{trimmed}/v1/chat/completions")
        }
    }

    fn conversation_to_messages(&self, prompt: &str, conversation: &[String]) -> Vec<ApiMessage> {
        let mut messages = Vec::with_capacity(conversation.len() + 2);

        messages.push(ApiMessage {
            role: "system".to_owned(),
            content: CORE_SYSTEM_PROMPT.to_owned(),
        });

        for line in conversation {
            if let Some(content) = line.strip_prefix("You:") {
                messages.push(ApiMessage {
                    role: "user".to_owned(),
                    content: content.trim().to_owned(),
                });
            } else if let Some(content) = line.strip_prefix("Auvro:") {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    messages.push(ApiMessage {
                        role: "assistant".to_owned(),
                        content: trimmed.to_owned(),
                    });
                }
            }
        }

        if !conversation
            .iter()
            .any(|line| line.trim_start().starts_with("You:"))
        {
            messages.push(ApiMessage {
                role: "user".to_owned(),
                content: prompt.to_owned(),
            });
        }

        messages
    }
}

impl Provider for HackClubProvider {
    fn name(&self) -> &str {
        "HackClub AI"
    }

    fn generate_reply(&self, prompt: &str, conversation: &[String]) -> Result<String, String> {
        let payload = ChatRequest {
            model: &self.model,
            messages: self.conversation_to_messages(prompt, conversation),
            temperature: 0.7,
        };

        let response = self
            .client
            .post(self.chat_endpoint())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .map_err(|err| format!("Request failed: {err}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("HackClub AI request failed ({status}): {text}"));
        }

        let body: ChatResponse = response
            .json()
            .map_err(|err| format!("Could not parse AI response: {err}"))?;

        body.message_content()
            .ok_or_else(|| "HackClub AI response did not include message content".to_owned())
    }
}

pub struct OpenRouterProvider {
    endpoint: String,
    api_key: String,
    model: String,
    site_url: Option<String>,
    app_name: Option<String>,
    client: Client,
}

impl OpenRouterProvider {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY").ok()?.trim().to_owned();
        if api_key.is_empty() {
            return None;
        }

        let endpoint = std::env::var("OPENROUTER_BASE_URL")
            .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_owned())
            .trim()
            .to_owned();

        let model = std::env::var("OPENROUTER_MODEL")
            .unwrap_or_else(|_| "meta-llama/llama-3.1-8b-instruct:free".to_owned())
            .trim()
            .to_owned();

        let site_url = std::env::var("OPENROUTER_SITE_URL")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        let app_name = std::env::var("OPENROUTER_APP_NAME")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .ok()?;

        Some(Self {
            endpoint,
            api_key,
            model,
            site_url,
            app_name,
            client,
        })
    }

    fn chat_endpoint(&self) -> String {
        let trimmed = self.endpoint.trim_end_matches('/');
        if trimmed.ends_with("/chat/completions") {
            trimmed.to_owned()
        } else {
            format!("{trimmed}/chat/completions")
        }
    }

    fn conversation_to_messages(&self, prompt: &str, conversation: &[String]) -> Vec<ApiMessage> {
        let mut messages = Vec::with_capacity(conversation.len() + 2);

        messages.push(ApiMessage {
            role: "system".to_owned(),
            content: CORE_SYSTEM_PROMPT.to_owned(),
        });

        for line in conversation {
            if let Some(content) = line.strip_prefix("You:") {
                messages.push(ApiMessage {
                    role: "user".to_owned(),
                    content: content.trim().to_owned(),
                });
            } else if let Some(content) = line.strip_prefix("Auvro:") {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    messages.push(ApiMessage {
                        role: "assistant".to_owned(),
                        content: trimmed.to_owned(),
                    });
                }
            }
        }

        if !conversation
            .iter()
            .any(|line| line.trim_start().starts_with("You:"))
        {
            messages.push(ApiMessage {
                role: "user".to_owned(),
                content: prompt.to_owned(),
            });
        }

        messages
    }
}

impl Provider for OpenRouterProvider {
    fn name(&self) -> &str {
        "OpenRouter"
    }

    fn generate_reply(&self, prompt: &str, conversation: &[String]) -> Result<String, String> {
        let payload = ChatRequest {
            model: &self.model,
            messages: self.conversation_to_messages(prompt, conversation),
            temperature: 0.7,
        };

        let mut request = self
            .client
            .post(self.chat_endpoint())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload);

        if let Some(site_url) = &self.site_url {
            request = request.header("HTTP-Referer", site_url);
        }
        if let Some(app_name) = &self.app_name {
            request = request.header("X-Title", app_name);
        }

        let response = request
            .send()
            .map_err(|err| format!("Request failed: {err}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("OpenRouter request failed ({status}): {text}"));
        }

        let body: ChatResponse = response
            .json()
            .map_err(|err| format!("Could not parse OpenRouter response: {err}"))?;

        body.message_content()
            .ok_or_else(|| "OpenRouter response did not include message content".to_owned())
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ApiMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Option<Vec<ChatChoice>>,
    content: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: Option<ChatMessage>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

impl ChatResponse {
    fn message_content(&self) -> Option<String> {
        if let Some(content) = &self.content {
            if !content.trim().is_empty() {
                return Some(content.trim().to_owned());
            }
        }

        if let Some(text) = &self.text {
            if !text.trim().is_empty() {
                return Some(text.trim().to_owned());
            }
        }

        self.choices
            .as_ref()
            .and_then(|choices| choices.first())
            .and_then(|choice| {
                choice
                    .message
                    .as_ref()
                    .and_then(|message| message.content.as_ref())
                    .cloned()
                    .or_else(|| choice.text.clone())
            })
            .map(|content| content.trim().to_owned())
            .filter(|content| !content.is_empty())
    }
}
