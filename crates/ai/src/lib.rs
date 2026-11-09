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
