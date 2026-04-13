use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;

const TITLE_PROMPT: &str = "Generate a short 4-6 word title for this conversation based on the first message. Reply with only the title, no punctuation, no quotes, no explanation.";
const MAX_TITLE_CHARS: usize = 60;

fn chat_endpoint() -> String {
    let trimmed = crate::env::AUVRO_ENDPOINT.trim().trim_end_matches('/');
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

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

fn extract_completion_text(body: &Value) -> Option<String> {
    let choices = body.get("choices")?.as_array()?;
    let first = choices.first()?;

    if let Some(content) = first
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
    {
        return Some(content.to_owned());
    }

    if let Some(parts) = first
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_array())
    {
        let merged = parts
            .iter()
            .filter_map(|part| part.get("text").and_then(|text| text.as_str()))
            .collect::<String>();
        if !merged.trim().is_empty() {
            return Some(merged);
        }
    }

    first
        .get("text")
        .and_then(|text| text.as_str())
        .map(str::to_owned)
}

pub fn generate_title(first_user_message: &str) -> Result<String, String> {
    let api_key = crate::env::AUVRO_API_KEY.trim();
    let model = crate::env::AUVRO_MODEL.trim();
    let endpoint = crate::env::AUVRO_ENDPOINT.trim();

    if api_key.is_empty() || model.is_empty() || endpoint.is_empty() {
        return Err("AUVRO endpoint/model/api key is not configured.".to_owned());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| format!("Failed to build title client: {err}"))?;

    let response = client
        .post(chat_endpoint())
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": TITLE_PROMPT },
                { "role": "user", "content": first_user_message }
            ],
            "stream": false,
            "temperature": 0.2
        }))
        .send()
        .map_err(|err| format!("Title request failed: {err}"))?;

    let status = response.status();
    let text = response.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("Title request failed ({status}): {text}"));
    }

    let body: Value = serde_json::from_str(&text)
        .map_err(|err| format!("Failed to parse title response: {err}"))?;

    let raw_title = extract_completion_text(&body)
        .ok_or_else(|| "Title response did not include text content.".to_owned())?;

    let normalized = collapse_whitespace(raw_title.trim()).trim_matches(|ch| ch == '"' || ch == '\'').to_owned();
    let truncated = truncate_chars(&normalized, MAX_TITLE_CHARS);
    if truncated.is_empty() {
        return Err("Generated title is empty.".to_owned());
    }

    Ok(truncated)
}