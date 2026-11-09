use std::fs;
use std::path::{Path, PathBuf};

use brainroom_core::{
    CURRENT_CHAT_VERSION, ChatRecord, ChatSummary, UserSettings, now_iso, now_millis,
};
use serde_json::json;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn default_dir() -> PathBuf {
        if let Some(dir) = std::env::var_os("BRAINROOM_DATA_DIR") {
            return PathBuf::from(dir);
        }
        if let Some(dir) = std::env::var_os("LH_DATA_DIR") {
            return PathBuf::from(dir);
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".config/Brainroom/data")
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn chats_dir(&self) -> PathBuf {
        self.root.join("chats")
    }

    pub fn settings_path(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    pub fn auth_path(&self) -> PathBuf {
        self.root
            .parent()
            .map(|parent| parent.join("auth.json"))
            .unwrap_or_else(|| self.root.join("auth.json"))
    }

    pub fn list_chats(&self) -> Vec<ChatSummary> {
        let Ok(entries) = fs::read_dir(self.chats_dir()) else {
            return Vec::new();
        };
        let mut chats: Vec<ChatSummary> = entries
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    return None;
                }
                let text = fs::read_to_string(&path).ok()?;
                let record: ChatRecord = serde_json::from_str(&text).ok()?;
                if record
                    .version
                    .is_some_and(|version| version > CURRENT_CHAT_VERSION)
                {
                    return None;
                }
                Some(record.summary())
            })
            .collect();
        chats.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        chats
    }

    pub fn load_chat(&self, id: &str) -> Option<ChatRecord> {
        if !valid_id(id) {
            return None;
        }
        let text = fs::read_to_string(self.chats_dir().join(format!("{id}.json"))).ok()?;
        let record: ChatRecord = serde_json::from_str(&text).ok()?;
        if record
            .version
            .is_some_and(|version| version > CURRENT_CHAT_VERSION)
        {
            return None;
        }
        Some(record)
    }

    pub fn create_chat(&self, title: &str) -> ChatRecord {
        let record = ChatRecord::new(new_chat_id(), title.trim().to_string());
        self.save_chat(&record);
        record
    }

    pub fn save_chat(&self, record: &ChatRecord) {
        let dir = self.chats_dir();
        let _ = fs::create_dir_all(&dir);
        let target = dir.join(format!("{}.json", record.id));
        let tmp = dir.join(format!(".{}.tmp-{}", record.id, Uuid::new_v4().simple()));
        if let Ok(text) = serde_json::to_string_pretty(record)
            && fs::write(&tmp, text).is_ok()
        {
            let _ = fs::rename(tmp, target);
        }
    }

    pub fn delete_chat(&self, id: &str) -> bool {
        if !valid_id(id) {
            return false;
        }
        fs::remove_file(self.chats_dir().join(format!("{id}.json"))).is_ok()
    }

    pub fn rename_chat(&self, id: &str, title: &str) -> Option<ChatRecord> {
        let mut record = self.load_chat(id)?;
        record.title = title.trim().to_owned();
        record.updated_at = now_iso();
        self.save_chat(&record);
        Some(record)
    }

    pub fn append_message(&self, record: &mut ChatRecord, role: &str, content: &str) {
        // LearnHarness stores user turns as a string and assistant turns as typed parts.
        let content = if role == "assistant" {
            json!([{ "type": "text", "text": content }])
        } else {
            json!(content)
        };
        let message = json!({
            "id": new_message_id(),
            "role": role,
            "content": content,
            "timestamp": now_millis(),
        });
        record.messages.push(message);
        record.updated_at = now_iso();
        record.version = Some(CURRENT_CHAT_VERSION);
        self.save_chat(record);
    }

    /// Append a fully-formed message (tool results, imported turns) and save.
    pub fn append_value(&self, record: &mut ChatRecord, message: serde_json::Value) {
        record.messages.push(message);
        record.updated_at = now_iso();
        record.version = Some(CURRENT_CHAT_VERSION);
        self.save_chat(record);
    }

    pub fn read_settings(&self) -> Option<UserSettings> {
        let text = fs::read_to_string(self.settings_path()).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn strong_model(&self) -> Option<brainroom_core::ModelRef> {
        self.read_settings()
            .and_then(|settings| settings.strong_model().cloned())
    }

    pub fn read_auth_key(&self, name: &str) -> Option<String> {
        let text = fs::read_to_string(self.auth_path()).ok()?;
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        value
            .get(name)
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn new_chat_id() -> String {
    format!(
        "chat_{}_{}",
        base36(now_millis()),
        &Uuid::new_v4().simple().to_string()[..8]
    )
}

fn new_message_id() -> String {
    format!(
        "msg_{}_{}",
        base36(now_millis()),
        &Uuid::new_v4().simple().to_string()[..8]
    )
}

fn base36(mut value: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if value == 0 {
        return "0".to_string();
    }
    let mut out = Vec::new();
    while value > 0 {
        out.push(DIGITS[(value % 36) as usize]);
        value /= 36;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> Store {
        let dir = std::env::temp_dir().join(format!("brainroom-store-{}", Uuid::new_v4()));
        Store::new(dir)
    }

    #[test]
    fn create_list_load_and_append() {
        let store = temp_store();
        let mut chat = store.create_chat("Gradient descent");
        let id = chat.id.clone();
        assert_eq!(store.list_chats().len(), 1);
        assert_eq!(store.load_chat(&id).unwrap().title, "Gradient descent");
        store.append_message(&mut chat, "user", "hello");
        store.append_message(&mut chat, "assistant", "hi there");
        let loaded = store.load_chat(&id).unwrap();
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0]["content"], "hello");
        assert_eq!(loaded.messages[1]["content"][0]["type"], "text");
        assert_eq!(loaded.messages[1]["content"][0]["text"], "hi there");
    }

    #[test]
    fn deletes_and_renames() {
        let store = temp_store();
        let chat = store.create_chat("old title");
        let renamed = store.rename_chat(&chat.id, "new title").unwrap();
        assert_eq!(renamed.title, "new title");
        assert_eq!(store.load_chat(&chat.id).unwrap().title, "new title");
        assert!(store.delete_chat(&chat.id));
        assert!(store.load_chat(&chat.id).is_none());
        assert!(!store.delete_chat("../auth"));
    }

    #[test]
    fn rejects_invalid_ids_and_future_versions() {
        let store = temp_store();
        assert!(store.load_chat("../auth").is_none());
        let mut chat = store.create_chat("future");
        chat.version = Some(CURRENT_CHAT_VERSION + 1);
        store.save_chat(&chat);
        assert!(store.load_chat(&chat.id).is_none());
    }
}
