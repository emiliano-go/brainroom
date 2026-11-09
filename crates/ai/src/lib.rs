//! OpenRouter transport shared by the Qt bridge and the Rune shell.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, anyhow, bail};
use futures_util::StreamExt;

/// A chat message: role (`system`, `user`, `assistant`, `tool`) and text content.
pub type Message = (String, String);

pub const SYSTEM_PROMPT: &str = "You are Brainroom, a concise, rigorous tutor. \
Ask one useful question at a time when the learner's understanding is unclear. \
Prefer explanation and a worked example before checking. Be honest about uncertainty. \
When you check understanding, call the quiz tool with one multiple-choice question instead of \
writing the question in prose. Record a multi-step lesson once with plan_init. Use \
list_sessions and list_propositions when the learner's history matters.";

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn read_json(path: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn auth_key(name: &str) -> Option<String> {
    if let Ok(key) = std::env::var(name)
        && !key.is_empty()
    {
        return Some(key);
    }
    for path in [
        home_dir().join(".config/LearnHarness/auth.json"),
        home_dir().join(".config/Brainroom/auth.json"),
    ] {
        if let Some(value) = read_json(&path)
            && let Some(key) = value.get(name).and_then(|v| v.as_str())
            && !key.is_empty()
        {
            return Some(key.to_owned());
        }
    }
    None
}

pub fn api_key() -> Option<String> {
    if let Ok(key) = std::env::var("OPENROUTER_API_KEY")
        && !key.is_empty()
    {
        return Some(key);
    }
    for path in [
        home_dir().join(".config/LearnHarness/auth.json"),
        home_dir().join(".config/Brainroom/auth.json"),
    ] {
        if let Some(value) = read_json(&path)
            && let Some(key) = value.get("OPENROUTER_API_KEY").and_then(|v| v.as_str())
            && !key.is_empty()
        {
            return Some(key.to_owned());
        }
    }
    None
}

/// The strong model from `settings.json` under `data_dir`, else a small free default.
pub fn model_id(data_dir: &Path) -> String {
    if let Some(id) = read_json(&data_dir.join("settings.json"))
        .and_then(|v| {
            v.pointer("/models/strong/model/id")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .filter(|id| !id.is_empty())
    {
        return id;
    }
    "nvidia/nemotron-3-ultra-550b-a55b".to_owned()
}

/// Best-effort context window for a model id. The catalog surface is static
/// until a provider catalog is wired; unknown models fall back to 202.8k.
pub fn context_window(model: &str) -> u64 {
    let model = model.to_lowercase();
    if model.contains("nemotron-3-ultra") {
        202_800
    } else if model.contains("claude") {
        200_000
    } else if model.contains("gpt-4.1") {
        1_047_576
    } else if model.contains("gpt-4o") || model.contains("o1") || model.contains("o3") {
        128_000
    } else if model.contains("gemini") {
        1_048_576
    } else if model.contains("llama") {
        131_072
    } else {
        202_800
    }
}

/// Fallback cost estimate when the provider omits `usage.cost`. Rates are
/// USD per million tokens.
pub fn estimated_cost(model: &str, prompt_tokens: u64, completion_tokens: u64) -> Option<f64> {
    let model = model.to_lowercase();
    let (input, output) = if model.contains("nemotron") {
        (0.6, 1.8)
    } else if model.contains("claude") {
        (3.0, 15.0)
    } else if model.contains("gpt-4.1") {
        (2.0, 8.0)
    } else if model.contains("gpt-4o") || model.contains("o1") || model.contains("o3") {
        (2.5, 10.0)
    } else if model.contains("gemini") {
        (1.25, 5.0)
    } else if model.contains("llama") {
        (0.2, 0.6)
    } else {
        return None;
    };
    Some((prompt_tokens as f64 * input + completion_tokens as f64 * output) / 1_000_000.0)
}

pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .expect("tokio runtime")
    })
}

/// Stream a completion, passing each text delta to `on_chunk`.
pub async fn stream_chat<F>(
    model: &str,
    messages: Vec<Message>,
    mut on_chunk: F,
) -> anyhow::Result<()>
where
    F: FnMut(&str) + Send,
{
    let key = api_key().ok_or_else(|| {
        anyhow!(
            "No OpenRouter API key found. Put OPENROUTER_API_KEY in \
             ~/.config/Brainroom/auth.json or ~/.config/LearnHarness/auth.json."
        )
    })?;
    let mut wire = vec![serde_json::json!({ "role": "system", "content": SYSTEM_PROMPT })];
    for (role, content) in messages {
        wire.push(serde_json::json!({ "role": role, "content": content }));
    }
    let body = serde_json::json!({
        "model": model,
        "stream": true,
        "max_tokens": 2048,
        "messages": wire
    });
    let response = reqwest::Client::new()
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {key}"))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://github.com/emiliano-go/brainroom")
        .header("X-Title", "Brainroom")
        .json(&body)
        .send()
        .await
        .context("OpenRouter request failed")?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        bail!("OpenRouter {status}: {text}");
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk.context("OpenRouter stream error")?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));
        while let Some(pos) = buffer.find('\n') {
            let line: String = buffer.drain(..=pos).collect();
            let Some(data) = line.trim().strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" {
                return Ok(());
            }
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(data)
                && let Some(text) = value
                    .pointer("/choices/0/delta/content")
                    .and_then(|v| v.as_str())
                && !text.is_empty()
            {
                on_chunk(text);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_windows_and_cost_fallbacks() {
        assert_eq!(context_window("nvidia/nemotron-3-ultra-550b-a55b"), 202_800);
        assert_eq!(context_window("anthropic/claude-haiku-5.5"), 200_000);
        assert_eq!(context_window("openai/gpt-4.1"), 1_047_576);
        let cost = estimated_cost("anthropic/claude-haiku-5.5", 1_000_000, 0).unwrap();
        assert!((cost - 3.0).abs() < 1e-9);
        assert!(estimated_cost("unknown/model", 1, 1).is_none());
    }
}
