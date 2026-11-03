use core::pin::Pin;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, anyhow, bail};
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use futures_util::StreamExt;

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status, cxx_name = "status")]
        #[qproperty(QString, version, cxx_name = "version")]
        #[qproperty(QString, initial_view, cxx_name = "initialView")]
        #[qproperty(QString, screenshot_path, cxx_name = "screenshotPath")]
        #[qproperty(QString, stream_text, cxx_name = "streamText")]
        #[qproperty(QString, error_text, cxx_name = "errorText")]
        #[qproperty(bool, busy, cxx_name = "busy")]
        #[qproperty(QString, initial_prompt, cxx_name = "initialPrompt")]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn boot(self: Pin<&mut Self>);

        #[qinvokable]
        fn send(self: Pin<&mut Self>, prompt: &QString);

        /// QML polls this at 10 Hz; it copies the background stream state
        /// into the Qt properties. Keeping all Qt-thread work here avoids
        /// cross-thread QObject access.
        #[qinvokable]
        fn poll(self: Pin<&mut Self>);
    }
}

#[derive(Default)]
struct ChatState {
    text: String,
    error: String,
    busy: bool,
}

pub struct AppControllerRust {
    status: QString,
    version: QString,
    initial_view: QString,
    screenshot_path: QString,
    stream_text: QString,
    error_text: QString,
    busy: bool,
    initial_prompt: QString,
    chat: Arc<Mutex<ChatState>>,
}

impl Default for AppControllerRust {
    fn default() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let initial_view = if args.iter().any(|arg| arg == "--home") {
            "home"
        } else {
            "session"
        };
        let screenshot_path = args
            .iter()
            .find_map(|arg| arg.strip_prefix("--screenshot="))
            .unwrap_or("");
        let initial_prompt = args
            .iter()
            .find_map(|arg| arg.strip_prefix("--prompt="))
            .unwrap_or("");
        Self {
            status: QString::from("Phase 0 · Rust + Qt"),
            version: QString::from(env!("CARGO_PKG_VERSION")),
            initial_view: QString::from(initial_view),
            screenshot_path: QString::from(screenshot_path),
            stream_text: QString::default(),
            error_text: QString::default(),
            busy: false,
            initial_prompt: QString::from(initial_prompt),
            chat: Arc::new(Mutex::new(ChatState::default())),
        }
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn read_json(path: &std::path::Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn api_key() -> Option<String> {
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
            return Some(key.to_string());
        }
    }
    None
}

fn model_id() -> String {
    let settings = home_dir().join(".config/LearnHarness/data/settings.json");
    if let Some(value) = read_json(&settings)
        && let Some(id) = value
            .pointer("/models/strong/model/id")
            .and_then(|v| v.as_str())
        && !id.is_empty()
    {
        return id.to_string();
    }
    "nvidia/nemotron-3-ultra-550b-a55b".to_string()
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .worker_threads(2)
            .build()
            .expect("tokio runtime")
    })
}

const SYSTEM_PROMPT: &str = "You are Brainroom, a concise, rigorous tutor. \
Ask one useful question at a time when the learner's understanding is unclear. \
Prefer explanation and a worked example before checking. Be honest about uncertainty.";

async fn stream_chat<F>(prompt: String, mut on_chunk: F) -> anyhow::Result<()>
where
    F: FnMut(&str) + Send,
{
    let key = api_key().ok_or_else(|| {
        anyhow!(
            "No OpenRouter API key found. Put OPENROUTER_API_KEY in \
             ~/.config/Brainroom/auth.json or ~/.config/LearnHarness/auth.json."
        )
    })?;
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model_id(),
        "stream": true,
        "max_tokens": 2048,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": prompt }
        ]
    });
    let response = client
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
            let line = line.trim();
            let Some(data) = line.strip_prefix("data:") else {
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

impl qobject::AppController {
    pub fn boot(mut self: Pin<&mut Self>) {
        self.as_mut().set_status(QString::from("Brainroom ready"));
    }

    pub fn send(mut self: Pin<&mut Self>, prompt: &QString) {
        let prompt = prompt.to_string();
        let state = self.as_ref().rust().chat.clone();
        {
            let mut chat = state.lock().expect("chat state");
            chat.text.clear();
            chat.error.clear();
            chat.busy = true;
        }
        self.as_mut().set_busy(true);
        self.as_mut().set_error_text(QString::default());
        self.as_mut().set_stream_text(QString::default());
        runtime().spawn(async move {
            let stream_state = state.clone();
            let result = stream_chat(prompt, move |delta| {
                if let Ok(mut chat) = stream_state.lock() {
                    chat.text.push_str(delta);
                }
            })
            .await;
            if let Ok(mut chat) = state.lock() {
                chat.busy = false;
                if let Err(error) = result {
                    chat.error = error.to_string();
                }
            }
        });
    }

    pub fn poll(mut self: Pin<&mut Self>) {
        let state = self.as_ref().rust().chat.clone();
        let (text, error, busy) = {
            let chat = state.lock().expect("chat state");
            (chat.text.clone(), chat.error.clone(), chat.busy)
        };
        self.as_mut().set_stream_text(QString::from(text));
        self.as_mut().set_error_text(QString::from(error));
        self.as_mut().set_busy(busy);
    }
}
