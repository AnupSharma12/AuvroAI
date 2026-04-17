use reqwest::Client;
use serde_json::Value;

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

async fn send_with_retry(
    build_request: impl Fn() -> reqwest::RequestBuilder,
    max_retries: u32,
) -> Result<reqwest::Response, String> {
    let mut attempts: u32 = 0;

    loop {
        match build_request().send().await {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) if resp.status().as_u16() == 429 => {
                if attempts >= max_retries {
                    return Err("Rate limited".to_owned());
                }
                let wait = 2u64.pow(attempts) * 1000;
                tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
                attempts += 1;
            }
            Ok(resp) => {
                return Err(format!("HTTP {}", resp.status()));
            }
            Err(err) if attempts < max_retries => {
                let wait = 2u64.pow(attempts) * 500;
                tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
                attempts += 1;
                let _ = err;
            }
            Err(err) => {
                return Err(format!("Network error: {err}"));
            }
        }
    }
}

pub fn extract_sse_delta_content(line: &str) -> Result<Option<String>, String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with(':') {
        return Ok(None);
    }

    let Some(payload) = trimmed.strip_prefix("data:") else {
        return Ok(None);
    };

    let payload = payload.trim_start();
    if payload.is_empty() || payload == "[DONE]" {
        return Ok(None);
    }

    let json: Value = serde_json::from_str(payload)
        .map_err(|err| format!("Could not parse streaming chunk: {err}"))?;

    Ok(json
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
        .and_then(|delta| delta.get("content"))
        .and_then(|content| content.as_str())
        .map(str::to_owned))
}

pub fn generate_title(client: &Client, first_user_message: &str) -> Result<String, String> {
    let api_key = crate::env::AUVRO_API_KEY.trim();
    let model = crate::env::AUVRO_MODEL.trim();
    let endpoint = crate::env::AUVRO_ENDPOINT.trim();

    if api_key.is_empty() || model.is_empty() || endpoint.is_empty() {
        return Err("AUVRO endpoint/model/api key is not configured.".to_owned());
    }

    let client = client.clone();

    let endpoint = chat_endpoint();
    let api_key = api_key.to_owned();
    let model = model.to_owned();
    let user_prompt = first_user_message.to_owned();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .map_err(|err| format!("Failed to initialize retry runtime: {err}"))?;

    let text = runtime.block_on(async {
        let response = send_with_retry(
            || {
                client
                    .post(&endpoint)
                    .header("Authorization", format!("Bearer {api_key}"))
                    .header("Content-Type", "application/json")
                    .json(&serde_json::json!({
                        "model": model,
                        "messages": [
                            { "role": "system", "content": TITLE_PROMPT },
                            { "role": "user", "content": user_prompt }
                        ],
                        "stream": false,
                        "temperature": 0.2
                    }))
            },
            3,
        )
        .await?;

        response
            .text()
            .await
            .map_err(|err| format!("Failed reading title response body: {err}"))
    })?;

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
