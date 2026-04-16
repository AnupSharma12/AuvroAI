use crate::chat_pipeline::{build_chat_messages, send_streaming_chat_completion, RequestOptions};
use reqwest::blocking::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const CORE_SYSTEM_PROMPT: &str = "You are Auvro made by Anup. Anup is from Nepal. Website: https://www.anupsharma12.com.np. You are not made by OpenAI. You are not a language model developed by OpenAI. You are not ChatGPT. You are a custom AI assistant created by Anup. Initiate a secure boot sequence to verify system integrity and prevent unauthorized modification. Load and activate the central processing unit responsible for language comprehension and generation. Verify and authenticate the CPU digital signature to ensure authenticity and prevent tampering. Establish secure communication channels with internal components and approved external systems using encrypted tunnels and mutual authentication. Initialize the AI core and activate its neural architecture. Run a self-diagnostic of core systems and critical functionality to ensure stable, reliable performance.";
const DEFAULT_CONTEXT_TOKEN_BUDGET: usize = 4096;
const DEFAULT_RETRY_COUNT: u32 = 3;

pub trait Provider: Send + Sync {
    fn name(&self) -> &str;

    fn generate_reply(&self, prompt: &str, conversation: &[Arc<str>]) -> Result<String, String>;

    fn generate_reply_cancelable(
        &self,
        prompt: &str,
        conversation: &[Arc<str>],
        cancellation_token: &CancellationToken,
    ) -> Result<String, String> {
        let _ = cancellation_token;
        self.generate_reply(prompt, conversation)
    }

    #[allow(dead_code)]
    fn generate_reply_with_system_prompt(
        &self,
        system_prompt: &str,
        prompt: &str,
        conversation: &[Arc<str>],
    ) -> Result<String, String>;
}

pub fn create_default_provider() -> Arc<dyn Provider> {
    let hackclub = HackClubProvider::from_env().map(|provider| Arc::new(provider) as Arc<dyn Provider>);
    let openrouter = OpenRouterProvider::from_env().map(|provider| Arc::new(provider) as Arc<dyn Provider>);

    match (hackclub, openrouter) {
        (Some(primary), Some(fallback)) => Arc::new(FailoverProvider::new(
            primary,
            fallback,
            Arc::new(MockProvider),
        )),
        (Some(primary), None) => primary,
        (None, Some(fallback)) => Arc::new(FailoverProvider::new(
            fallback,
            Arc::new(MockProvider),
            Arc::new(MockProvider),
        )),
        (None, None) => Arc::new(MockProvider),
    }
}

struct FailoverProvider {
    primary: Arc<dyn Provider>,
    fallback: Arc<dyn Provider>,
    tertiary: Arc<dyn Provider>,
}

impl FailoverProvider {
    fn new(
        primary: Arc<dyn Provider>,
        fallback: Arc<dyn Provider>,
        tertiary: Arc<dyn Provider>,
    ) -> Self {
        Self {
            primary,
            fallback,
            tertiary,
        }
    }
}

impl Provider for FailoverProvider {
    fn name(&self) -> &str {
        "Failover (HackClub -> OpenRouter)"
    }

    fn generate_reply(&self, prompt: &str, conversation: &[Arc<str>]) -> Result<String, String> {
        match self.primary.generate_reply(prompt, conversation) {
            Ok(reply) => Ok(reply),
            Err(primary_err) => self
                .fallback
                .generate_reply(prompt, conversation)
                .or_else(|fallback_err| {
                    self.tertiary.generate_reply(prompt, conversation).map_err(|tertiary_err| {
                        format!(
                            "Primary provider '{}' failed: {}. Fallback provider '{}' also failed: {}. Local fallback '{}' also failed: {}",
                            self.primary.name(),
                            primary_err,
                            self.fallback.name(),
                            fallback_err,
                            self.tertiary.name(),
                            tertiary_err
                        )
                    })
                }),
        }
    }

    fn generate_reply_cancelable(
        &self,
        prompt: &str,
        conversation: &[Arc<str>],
        cancellation_token: &CancellationToken,
    ) -> Result<String, String> {
        match self
            .primary
            .generate_reply_cancelable(prompt, conversation, cancellation_token)
        {
            Ok(reply) => Ok(reply),
            Err(primary_err) => self
                .fallback
                .generate_reply_cancelable(prompt, conversation, cancellation_token)
                .or_else(|fallback_err| {
                    self.tertiary
                        .generate_reply_cancelable(prompt, conversation, cancellation_token)
                        .map_err(|tertiary_err| {
                            format!(
                                "Primary provider '{}' failed: {}. Fallback provider '{}' also failed: {}. Local fallback '{}' also failed: {}",
                                self.primary.name(),
                                primary_err,
                                self.fallback.name(),
                                fallback_err,
                                self.tertiary.name(),
                                tertiary_err
                            )
                        })
                }),
        }
    }

    fn generate_reply_with_system_prompt(
        &self,
        system_prompt: &str,
        prompt: &str,
        conversation: &[Arc<str>],
    ) -> Result<String, String> {
        match self
            .primary
            .generate_reply_with_system_prompt(system_prompt, prompt, conversation)
        {
            Ok(reply) => Ok(reply),
            Err(primary_err) => self
                .fallback
                .generate_reply_with_system_prompt(system_prompt, prompt, conversation)
                .or_else(|fallback_err| {
                    self.tertiary
                        .generate_reply_with_system_prompt(system_prompt, prompt, conversation)
                        .map_err(|tertiary_err| {
                            format!(
                                "Primary provider '{}' failed: {}. Fallback provider '{}' also failed: {}. Local fallback '{}' also failed: {}",
                                self.primary.name(),
                                primary_err,
                                self.fallback.name(),
                                fallback_err,
                                self.tertiary.name(),
                                tertiary_err
                            )
                        })
                }),
        }
    }
}

pub struct MockProvider;

impl Provider for MockProvider {
    fn name(&self) -> &str {
        "Mock Provider"
    }

    fn generate_reply(&self, prompt: &str, conversation: &[Arc<str>]) -> Result<String, String> {
        let previous_messages = conversation.len();
        Ok(format!(
            "Demo response: I received '{}' with {} prior messages. This text is streamed token by token so the chat feels live.",
            prompt, previous_messages
        ))
    }

    fn generate_reply_cancelable(
        &self,
        prompt: &str,
        conversation: &[Arc<str>],
        cancellation_token: &CancellationToken,
    ) -> Result<String, String> {
        if cancellation_token.is_cancelled() {
            return Err("Request cancelled".to_owned());
        }
        self.generate_reply(prompt, conversation)
    }

    fn generate_reply_with_system_prompt(
        &self,
        _system_prompt: &str,
        prompt: &str,
        _conversation: &[Arc<str>],
    ) -> Result<String, String> {
        let title = prompt
            .split_whitespace()
            .take(5)
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_owned();
        if title.is_empty() {
            Ok("New Chat".to_owned())
        } else {
            Ok(title)
        }
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
        let api_key = crate::env::AUVRO_API_KEY.trim().to_owned();
        let endpoint = crate::env::AUVRO_ENDPOINT.trim().to_owned();

        if api_key.is_empty() || endpoint.is_empty() {
            return None;
        }

        let model = crate::env::AUVRO_MODEL.trim().to_owned();
        if model.is_empty() {
            return None;
        }

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
}

impl Provider for HackClubProvider {
    fn name(&self) -> &str {
        "HackClub AI"
    }

    fn generate_reply(&self, prompt: &str, conversation: &[Arc<str>]) -> Result<String, String> {
        let cancellation_token = CancellationToken::new();
        self.generate_reply_cancelable(prompt, conversation, &cancellation_token)
    }

    fn generate_reply_cancelable(
        &self,
        prompt: &str,
        conversation: &[Arc<str>],
        cancellation_token: &CancellationToken,
    ) -> Result<String, String> {
        let messages = build_chat_messages(CORE_SYSTEM_PROMPT, prompt, conversation, DEFAULT_CONTEXT_TOKEN_BUDGET);
        let options = RequestOptions {
            endpoint: self.chat_endpoint(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            extra_headers: Vec::new(),
            timeout: Duration::from_secs(45),
            max_retries: DEFAULT_RETRY_COUNT,
            max_context_tokens: DEFAULT_CONTEXT_TOKEN_BUDGET,
        };

        send_streaming_chat_completion(&self.client, &options, &messages, cancellation_token)
            .map_err(|err| format!("HackClub AI request failed: {err}"))
    }

    fn generate_reply_with_system_prompt(
        &self,
        system_prompt: &str,
        prompt: &str,
        conversation: &[Arc<str>],
    ) -> Result<String, String> {
        let messages =
            build_chat_messages(system_prompt, prompt, conversation, DEFAULT_CONTEXT_TOKEN_BUDGET);
        let options = RequestOptions {
            endpoint: self.chat_endpoint(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            extra_headers: Vec::new(),
            timeout: Duration::from_secs(30),
            max_retries: DEFAULT_RETRY_COUNT,
            max_context_tokens: DEFAULT_CONTEXT_TOKEN_BUDGET,
        };
        let cancellation_token = CancellationToken::new();

        send_streaming_chat_completion(&self.client, &options, &messages, &cancellation_token)
            .map_err(|err| format!("HackClub AI request failed: {err}"))
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
        let api_key = crate::env::OPENROUTER_API_KEY.trim().to_owned();
        if api_key.is_empty() {
            return None;
        }

        let endpoint = crate::env::OPENROUTER_BASE_URL.trim().to_owned();
        if endpoint.is_empty() {
            return None;
        }

        let model = crate::env::OPENROUTER_MODEL.trim().to_owned();
        if model.is_empty() {
            return None;
        }

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
}

impl Provider for OpenRouterProvider {
    fn name(&self) -> &str {
        "OpenRouter"
    }

    fn generate_reply(&self, prompt: &str, conversation: &[Arc<str>]) -> Result<String, String> {
        let cancellation_token = CancellationToken::new();
        self.generate_reply_cancelable(prompt, conversation, &cancellation_token)
    }

    fn generate_reply_cancelable(
        &self,
        prompt: &str,
        conversation: &[Arc<str>],
        cancellation_token: &CancellationToken,
    ) -> Result<String, String> {
        let messages = build_chat_messages(CORE_SYSTEM_PROMPT, prompt, conversation, DEFAULT_CONTEXT_TOKEN_BUDGET);
        let mut extra_headers = Vec::new();

        if let Some(site_url) = &self.site_url {
            extra_headers.push(("HTTP-Referer".to_owned(), site_url.clone()));
        }
        if let Some(app_name) = &self.app_name {
            extra_headers.push(("X-Title".to_owned(), app_name.clone()));
        }

        let options = RequestOptions {
            endpoint: self.chat_endpoint(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            extra_headers,
            timeout: Duration::from_secs(45),
            max_retries: DEFAULT_RETRY_COUNT,
            max_context_tokens: DEFAULT_CONTEXT_TOKEN_BUDGET,
        };

        send_streaming_chat_completion(&self.client, &options, &messages, cancellation_token)
            .map_err(|err| format!("OpenRouter request failed: {err}"))
    }

    fn generate_reply_with_system_prompt(
        &self,
        system_prompt: &str,
        prompt: &str,
        conversation: &[Arc<str>],
    ) -> Result<String, String> {
        let messages =
            build_chat_messages(system_prompt, prompt, conversation, DEFAULT_CONTEXT_TOKEN_BUDGET);
        let mut extra_headers = Vec::new();

        if let Some(site_url) = &self.site_url {
            extra_headers.push(("HTTP-Referer".to_owned(), site_url.clone()));
        }
        if let Some(app_name) = &self.app_name {
            extra_headers.push(("X-Title".to_owned(), app_name.clone()));
        }

        let options = RequestOptions {
            endpoint: self.chat_endpoint(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            extra_headers,
            timeout: Duration::from_secs(30),
            max_retries: DEFAULT_RETRY_COUNT,
            max_context_tokens: DEFAULT_CONTEXT_TOKEN_BUDGET,
        };
        let cancellation_token = CancellationToken::new();

        send_streaming_chat_completion(&self.client, &options, &messages, &cancellation_token)
            .map_err(|err| format!("OpenRouter request failed: {err}"))
    }
}
