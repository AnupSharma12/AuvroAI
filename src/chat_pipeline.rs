use reqwest::blocking::{Client, Response};
use serde::Serialize;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const DEFAULT_CONTEXT_TOKEN_BUDGET: usize = 4096;
const DEFAULT_RETRY_COUNT: u32 = 3;
const DEFAULT_BACKOFF_BASE_MS: u64 = 250;

#[derive(Clone, Debug, Serialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug)]
pub struct RequestOptions {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    pub extra_headers: Vec<(String, String)>,
    pub timeout: Duration,
    pub max_retries: u32,
    pub max_context_tokens: usize,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ApiMessage],
    stream: bool,
    temperature: f32,
}

pub fn build_chat_messages(
    system_prompt: &str,
    prompt: &str,
    conversation: &[String],
    max_context_tokens: usize,
) -> Vec<ApiMessage> {
    let mut history = Vec::new();

    for line in conversation {
        if let Some(content) = line.strip_prefix("You:") {
            history.push(ApiMessage {
                role: "user".to_owned(),
                content: content.trim().to_owned(),
            });
        } else if let Some(content) = line.strip_prefix("Auvro:") {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                history.push(ApiMessage {
                    role: "assistant".to_owned(),
                    content: trimmed.to_owned(),
                });
            }
        }
    }

    let reserve_tokens = approximate_tokens(system_prompt) + approximate_tokens(prompt) + 32;
    let available_tokens = max_context_tokens.saturating_sub(reserve_tokens);

    let mut selected_history = Vec::new();
    let mut remaining_tokens = available_tokens;

    for message in history.into_iter().rev() {
        let message_tokens = approximate_message_tokens(&message);
        if message_tokens <= remaining_tokens {
            remaining_tokens -= message_tokens;
            selected_history.push(message);
        } else {
            break;
        }
    }

    selected_history.reverse();

    let mut messages = Vec::with_capacity(selected_history.len() + 2);
    messages.push(ApiMessage {
        role: "system".to_owned(),
        content: system_prompt.to_owned(),
    });
    messages.extend(selected_history);
    messages.push(ApiMessage {
        role: "user".to_owned(),
        content: prompt.to_owned(),
    });
    messages
}

pub fn send_streaming_chat_completion(
    client: &Client,
    options: &RequestOptions,
    messages: &[ApiMessage],
    cancellation_token: &CancellationToken,
) -> Result<String, String> {
    let mut last_error: Option<String> = None;

    for attempt in 0..=options.max_retries {
        if cancellation_token.is_cancelled() {
            return Err("Request cancelled".to_owned());
        }

        match send_once(client, options, messages, cancellation_token) {
            Ok(reply) => return Ok(reply),
            Err(err) => {
                last_error = Some(err.clone());
                if attempt >= options.max_retries || !is_transient_error(&err) {
                    break;
                }

                let backoff_ms = DEFAULT_BACKOFF_BASE_MS.saturating_mul(2u64.saturating_pow(attempt));
                wait_with_cancellation(cancellation_token, Duration::from_millis(backoff_ms))?;
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "Request failed".to_owned()))
}

fn send_once(
    client: &Client,
    options: &RequestOptions,
    messages: &[ApiMessage],
    cancellation_token: &CancellationToken,
) -> Result<String, String> {
    let payload = ChatRequest {
        model: &options.model,
        messages,
        stream: true,
        temperature: 0.7,
    };

    let mut request = client
        .post(chat_endpoint(&options.endpoint))
        .timeout(options.timeout)
        .header("Authorization", format!("Bearer {}", options.api_key))
        .header("Content-Type", "application/json")
        .json(&payload);

    for (key, value) in &options.extra_headers {
        request = request.header(key, value);
    }

    let response = request
        .send()
        .map_err(|err| format!("Request failed: {err}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }

    read_streaming_response(response, cancellation_token)
}

fn read_streaming_response(
    response: Response,
    cancellation_token: &CancellationToken,
) -> Result<String, String> {
    let mut reader = BufReader::new(response);
    let mut output = String::new();
    let mut buffer = Vec::new();

    loop {
        if cancellation_token.is_cancelled() {
            return Err("Request cancelled".to_owned());
        }

        buffer.clear();
        let bytes_read = reader
            .read_until(b'\n', &mut buffer)
            .map_err(|err| format!("Stream read failed: {err}"))?;

        if bytes_read == 0 {
            break;
        }

        if buffer.ends_with(b"\n") {
            buffer.pop();
        }
        if buffer.ends_with(b"\r") {
            buffer.pop();
        }

        if buffer.is_empty() {
            continue;
        }

        let Some(payload) = sse_data_payload(&buffer) else {
            continue;
        };

        if payload == b"[DONE]" {
            break;
        }

        let delta = extract_chunk_text(payload)?;
        if !delta.is_empty() {
            output.push_str(&delta);
        }
    }

    Ok(output)
}

fn sse_data_payload(line: &[u8]) -> Option<&[u8]> {
    if line.starts_with(b":") {
        return None;
    }

    let payload = if let Some(payload) = line.strip_prefix(b"data:") {
        trim_ascii_start(payload)
    } else {
        return None;
    };

    if payload.is_empty() {
        None
    } else {
        Some(payload)
    }
}

fn trim_ascii_start(mut bytes: &[u8]) -> &[u8] {
    while let Some((first, rest)) = bytes.split_first() {
        if first.is_ascii_whitespace() {
            bytes = rest;
        } else {
            break;
        }
    }

    bytes
}

fn extract_chunk_text(payload: &[u8]) -> Result<String, String> {
    let Some(first) = payload.first() else {
        return Ok(String::new());
    };
    if *first != b'{' && *first != b'[' {
        // Some providers send plain-text keepalive/status lines in SSE.
        return Ok(String::new());
    }

    let json: Value = serde_json::from_slice(payload)
        .map_err(|err| format!("Could not parse streaming chunk: {err}"))?;

    if let Some(content) = json
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("content"))
        .and_then(|content| content.as_str())
    {
        return Ok(content.to_owned());
    }

    if let Some(content) = json
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
    {
        return Ok(content.to_owned());
    }

    if let Some(text) = json.get("text").and_then(|value| value.as_str()) {
        return Ok(text.to_owned());
    }

    Ok(String::new())
}

fn chat_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

fn approximate_message_tokens(message: &ApiMessage) -> usize {
    approximate_tokens(&message.content) + 4
}

fn approximate_tokens(text: &str) -> usize {
    let whitespace_tokens = text.split_whitespace().count().max(1);
    let char_tokens = text.chars().count().div_ceil(4).max(1);
    whitespace_tokens.max(char_tokens)
}

fn is_transient_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("timeout")
        || lower.contains("temporarily")
        || lower.contains("connection reset")
        || lower.contains("connection refused")
        || lower.contains("429")
        || lower.contains("500")
        || lower.contains("502")
        || lower.contains("503")
        || lower.contains("504")
        || lower.contains("request failed")
}

fn wait_with_cancellation(
    cancellation_token: &CancellationToken,
    duration: Duration,
) -> Result<(), String> {
    let slice = Duration::from_millis(25);
    let mut waited = Duration::from_millis(0);

    while waited < duration {
        if cancellation_token.is_cancelled() {
            return Err("Request cancelled".to_owned());
        }

        let step = slice.min(duration - waited);
        std::thread::sleep(step);
        waited += step;
    }

    Ok(())
}
