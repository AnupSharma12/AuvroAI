// Before: app startup could eagerly fetch model metadata, increasing cold-start latency.
// After: metadata is lazy-loaded only when Settings opens, then cached in memory with TTL.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const ALL_PROVIDERS_UNAVAILABLE_MESSAGE: &str = "Could not load models — HackClub AI and OpenRouter both unavailable. Check your connection or add an OpenRouter key in Settings.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelProvider {
    HackClub,
    OpenRouter,
}

#[derive(Clone, Debug)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub provider: ModelProvider,
    pub context_length: u32,
    pub prompt_price_per_1k: f64,
    pub completion_price_per_1k: f64,
}

#[derive(Clone, Debug)]
pub enum CacheState {
    Empty,
    Loading,
    Loaded { models: Vec<ModelInfo>, fetched_at: Instant },
    Failed(String),
}

pub struct ModelMetadataCache {
    state: Arc<Mutex<CacheState>>,
    ttl: Duration,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl ModelMetadataCache {
    pub fn new(ttl: Duration) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime creation failed for model metadata cache");

        Self {
            state: Arc::new(Mutex::new(CacheState::Empty)),
            ttl,
            runtime: Arc::new(runtime),
        }
    }

    pub fn ensure_loaded(&self, openrouter_api_key: &str) -> bool {
        let now = Instant::now();
        let should_fetch = {
            let guard = match self.state.lock() {
                Ok(g) => g,
                Err(_) => return false,
            };

            match &*guard {
                CacheState::Empty => true,
                CacheState::Loading => return false,
                CacheState::Loaded { fetched_at, .. } => now.duration_since(*fetched_at) >= self.ttl,
                CacheState::Failed(_) => return false,
            }
        };

        if should_fetch {
            if let Ok(mut guard) = self.state.lock() {
                *guard = CacheState::Loading;
            }

            let openrouter_api_key = openrouter_api_key.trim().to_owned();
            let state = Arc::clone(&self.state);
            self.runtime.spawn(async move {
                let result = fetch_models_with_fallback(&openrouter_api_key).await;
                if let Ok(mut guard) = state.lock() {
                    *guard = match result {
                        Ok(models) => CacheState::Loaded {
                            models,
                            fetched_at: Instant::now(),
                        },
                        Err(err) => CacheState::Failed(err),
                    };
                }
            });

            return false;
        }

        self.get_models().is_some()
    }

    pub fn get_models(&self) -> Option<Vec<ModelInfo>> {
        let guard = self.state.lock().ok()?;
        match &*guard {
            CacheState::Loaded { models, fetched_at } => {
                if fetched_at.elapsed() <= self.ttl {
                    Some(models.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn invalidate(&self) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = CacheState::Empty;
        }
    }

    pub fn set_failed(&self, message: String) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = CacheState::Failed(message);
        }
    }

    pub fn state_snapshot(&self) -> CacheState {
        match self.state.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => CacheState::Failed("Cache state unavailable".to_owned()),
        }
    }
}

fn parse_price(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn parse_models(body: Value, provider: ModelProvider, parse_error_label: &str) -> Result<Vec<ModelInfo>, String> {
    let entries = if let Some(arr) = body.as_array() {
        arr
    } else if let Some(arr) = body.get("data").and_then(|d| d.as_array()) {
        arr
    } else {
        return Err(parse_error_label.to_owned());
    };

    let mut models = Vec::with_capacity(entries.len());
    for item in entries {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();
        if id.is_empty() {
            continue;
        }

        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| id.clone());

        let context_length = item.get("context_length").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let pricing = item.get("pricing");
        let prompt_price_per_1k = parse_price(pricing.and_then(|p| p.get("prompt")));
        let completion_price_per_1k = parse_price(pricing.and_then(|p| p.get("completion")));

        models.push(ModelInfo {
            id,
            name,
            provider,
            context_length,
            prompt_price_per_1k,
            completion_price_per_1k,
        });
    }

    Ok(models)
}

async fn fetch_models_with_fallback(openrouter_api_key: &str) -> Result<Vec<ModelInfo>, String> {
    if let Ok(models) = fetch_models_hackclub().await {
        if !models.is_empty() {
            return Ok(models);
        }
    }

    if openrouter_api_key.trim().is_empty() {
        return Err(ALL_PROVIDERS_UNAVAILABLE_MESSAGE.to_owned());
    }

    match fetch_models_openrouter(openrouter_api_key).await {
        Ok(models) if !models.is_empty() => Ok(models),
        Ok(_) => Err(ALL_PROVIDERS_UNAVAILABLE_MESSAGE.to_owned()),
        Err(_) => Err(ALL_PROVIDERS_UNAVAILABLE_MESSAGE.to_owned()),
    }
}

async fn fetch_models_hackclub() -> Result<Vec<ModelInfo>, String> {
    let base = crate::env::AUVRO_ENDPOINT.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("HackClub endpoint is missing".to_owned());
    }
    let endpoint = if base.ends_with("/models") {
        base.to_owned()
    } else {
        format!("{base}/models")
    };

    let mut headers = HeaderMap::new();
    headers.insert("HTTP-Referer", HeaderValue::from_static("AuvroAI"));
    headers.insert("X-Title", HeaderValue::from_static("AuvroAI"));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .default_headers(headers)
        .build()
        .map_err(|_| "HackClub request setup failed".to_owned())?;

    let response = client
        .get(endpoint)
        .send()
        .await
        .map_err(|err| {
            if err.is_timeout() {
                "HackClub request timed out".to_owned()
            } else {
                format!("HackClub network error: {err}")
            }
        })?;

    if !response.status().is_success() {
        return Err(format!("HackClub request failed ({})", response.status()));
    }

    let body: Value = response
        .json()
        .await
        .map_err(|_| "Unexpected response from HackClub AI".to_owned())?;

    parse_models(body, ModelProvider::HackClub, "Unexpected response from HackClub AI")
}

async fn fetch_models_openrouter(openrouter_api_key: &str) -> Result<Vec<ModelInfo>, String> {
    let base = crate::env::OPENROUTER_BASE_URL.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err("Invalid API key — check Settings".to_owned());
    }
    let endpoint = if base.ends_with("/models") {
        base.to_owned()
    } else {
        format!("{base}/models")
    };

    let mut headers = HeaderMap::new();
    let auth_value = format!("Bearer {openrouter_api_key}");
    let auth = HeaderValue::from_str(&auth_value)
        .map_err(|_| "Invalid API key — check Settings".to_owned())?;
    headers.insert(AUTHORIZATION, auth);
    headers.insert("HTTP-Referer", HeaderValue::from_static("AuvroAI"));
    headers.insert("X-Title", HeaderValue::from_static("AuvroAI"));

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .default_headers(headers)
        .build()
        .map_err(|_| "Request timed out".to_owned())?;

    let response = client
        .get(endpoint)
        .send()
        .await
        .map_err(|err| {
            if err.is_timeout() {
                "Request timed out".to_owned()
            } else {
                format!("Network error: {err}")
            }
        })?;

    if !response.status().is_success() {
        return Err("Invalid API key — check Settings".to_owned());
    }

    let body: Value = response
        .json()
        .await
        .map_err(|_| "Unexpected response from OpenRouter".to_owned())?;

    parse_models(body, ModelProvider::OpenRouter, "Unexpected response from OpenRouter")
}
