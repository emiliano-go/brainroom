use core::pin::Pin;
use std::sync::{Arc, Mutex};

use brainroom_ai as ai;
use brainroom_core::ChatRecord;
use brainroom_store::Store;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

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
        #[qproperty(QString, sessions_json, cxx_name = "sessionsJson")]
        #[qproperty(QString, initial_prompt, cxx_name = "initialPrompt")]
        type AppController = super::AppControllerRust;

        #[qinvokable]
        fn boot(self: Pin<&mut Self>);

        #[qinvokable]
        fn send(self: Pin<&mut Self>, prompt: &QString);

        #[qinvokable]
        fn open_chat(self: Pin<&mut Self>, id: &QString);

        #[qinvokable]
        fn new_chat(self: Pin<&mut Self>);

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
    sessions_json: QString,
    initial_prompt: QString,
    chat: Arc<Mutex<ChatState>>,
    store: Store,
    active_chat: Arc<Mutex<Option<String>>>,
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
        let store = Store::new(Store::default_dir());
        let sessions_json = sessions_json(&store);
        Self {
            status: QString::from("Phase 0 · Rust + Qt"),
            version: QString::from(env!("CARGO_PKG_VERSION")),
            initial_view: QString::from(initial_view),
            screenshot_path: QString::from(screenshot_path),
            stream_text: QString::default(),
            error_text: QString::default(),
            busy: false,
            sessions_json: QString::from(sessions_json),
            initial_prompt: QString::from(initial_prompt),
            chat: Arc::new(Mutex::new(ChatState::default())),
            store,
            active_chat: Arc::new(Mutex::new(None)),
        }
    }
}

fn message_text(message: &serde_json::Value) -> String {
    if let Some(text) = message.get("content").and_then(|value| value.as_str()) {
        return text.to_string();
    }
    if let Some(parts) = message.get("content").and_then(|value| value.as_array()) {
        return parts
            .iter()
            .filter_map(|part| part.get("text").and_then(|value| value.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
    }
    String::new()
}

fn sessions_json(store: &Store) -> String {
    serde_json::to_string(&store.list_chats()).unwrap_or_else(|_| "[]".to_string())
}

fn title_from_prompt(prompt: &str) -> String {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return "Untitled session".to_string();
    }
    let text: String = trimmed.chars().take(64).collect();
    if trimmed.chars().count() > 64 {
        format!("{text}…")
    } else {
        text
    }
}

impl qobject::AppController {
    pub fn boot(mut self: Pin<&mut Self>) {
        self.as_mut().set_status(QString::from("Brainroom ready"));
    }

    pub fn send(mut self: Pin<&mut Self>, prompt: &QString) {
        let prompt = prompt.to_string();
        let store = self.as_ref().rust().store.clone();
        let active = self.as_ref().rust().active_chat.clone();
        let mut record: ChatRecord = {
            let mut guard = active.lock().expect("active chat");
            let existing = guard.clone();
            existing
                .and_then(|id| store.load_chat(&id))
                .unwrap_or_else(|| {
                    let record = store.create_chat(&title_from_prompt(&prompt));
                    *guard = Some(record.id.clone());
                    record
                })
        };
        let chat_id = record.id.clone();
        store.append_message(&mut record, "user", &prompt);

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
        let data_dir = Store::default_dir();
        let lh_dir = ai::home_dir().join(".config/LearnHarness/data");
        let model = Store::new(data_dir.clone())
            .strong_model()
            .map(|m| m.id)
            .unwrap_or_else(|| ai::model_id(&lh_dir));
        ai::runtime().spawn(async move {
            let stream_state = state.clone();
            let result = ai::stream_chat(&model, vec![("user".to_owned(), prompt)], move |delta| {
                if let Ok(mut chat) = stream_state.lock() {
                    chat.text.push_str(delta);
                }
            })
            .await;
            let final_text = {
                let mut chat = state.lock().expect("chat state");
                chat.busy = false;
                if let Err(error) = &result {
                    chat.error = error.to_string();
                }
                chat.text.clone()
            };
            if result.is_ok()
                && !final_text.is_empty()
                && let Some(mut record) = store.load_chat(&chat_id)
            {
                store.append_message(&mut record, "assistant", &final_text);
            }
        });
    }

    pub fn new_chat(mut self: Pin<&mut Self>) {
        *self
            .as_ref()
            .rust()
            .active_chat
            .lock()
            .expect("active chat") = None;
        let state = self.as_ref().rust().chat.clone();
        {
            let mut chat = state.lock().expect("chat state");
            chat.text.clear();
            chat.error.clear();
            chat.busy = false;
        }
        self.as_mut().set_stream_text(QString::default());
        self.as_mut().set_error_text(QString::default());
        self.as_mut().set_busy(false);
    }

    pub fn open_chat(mut self: Pin<&mut Self>, id: &QString) {
        let id = id.to_string();
        let store = self.as_ref().rust().store.clone();
        let Some(record) = store.load_chat(&id) else {
            return;
        };
        *self
            .as_ref()
            .rust()
            .active_chat
            .lock()
            .expect("active chat") = Some(id);
        let transcript = record
            .messages
            .iter()
            .map(message_text)
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let state = self.as_ref().rust().chat.clone();
        {
            let mut chat = state.lock().expect("chat state");
            chat.text = transcript;
            chat.error.clear();
            chat.busy = false;
        }
        self.as_mut().set_error_text(QString::default());
        self.as_mut().set_busy(false);
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
        let sessions = sessions_json(&self.as_ref().rust().store);
        self.as_mut().set_sessions_json(QString::from(sessions));
    }
}
