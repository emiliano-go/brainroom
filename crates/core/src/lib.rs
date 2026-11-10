pub mod compaction;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const CURRENT_CHAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ModelRef {
    pub provider: String,
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChatSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatRecord {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub messages: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<ChatSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// Unknown fields (tree, turns, extractions, …) are preserved verbatim
    /// on a read-modify-write so a future LearnHarness record survives a
    /// Brainroom edit.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub fn normalize_kind(kind: Option<&str>) -> &'static str {
    match kind {
        Some("practice") => "practice",
        _ => "teach",
    }
}

fn summary_state(messages: &[Value]) -> (String, bool, bool) {
    let mut phase = "probe";
    let mut tagged_check = false;
    let mut probe_done = false;
    let mut pending_question = false;
    let mut pending_problem = false;
    let mut answered = std::collections::HashSet::new();
    for message in messages {
        if message.get("role").and_then(Value::as_str) == Some("toolResult") {
            if message.get("isError").and_then(Value::as_bool) != Some(true)
                && message.get("toolName").and_then(Value::as_str) == Some("probe_done")
            {
                probe_done = true;
            }
            if let Some(id) = message.get("toolCallId").and_then(Value::as_str) {
                answered.insert(id.to_owned());
            }
            continue;
        }
        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let Some(parts) = message.get("content").and_then(Value::as_array) else {
            continue;
        };
        for part in parts {
            if part.get("type").and_then(Value::as_str) != Some("toolCall") {
                continue;
            }
            let name = part.get("name").and_then(Value::as_str).unwrap_or("");
            let id = part.get("id").and_then(Value::as_str).unwrap_or("");
            match name {
                "quiz" | "explain" => {
                    let node = part
                        .get("arguments")
                        .and_then(|a| a.get("nodeId"))
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    if !node.is_empty() {
                        tagged_check = true;
                    }
                    if !id.is_empty() && !answered.contains(id) {
                        pending_question = true;
                    }
                }
                "probe" | "recall" | "ask_user_question" => {
                    if !id.is_empty() && !answered.contains(id) {
                        pending_question = true;
                    }
                }
                "problem" => pending_problem = true,
                "conclude" => pending_problem = false,
                _ => {}
            }
        }
    }
    if probe_done {
        phase = "plan";
    }
    if tagged_check {
        phase = "teach";
    }
    (phase.to_owned(), pending_question, pending_problem)
}

impl ChatRecord {
    pub fn new(id: String, title: String) -> Self {
        let now = now_iso();
        Self {
            id,
            title,
            created_at: now.clone(),
            updated_at: now,
            messages: Vec::new(),
            kind: None,
            settings: None,
            version: Some(CURRENT_CHAT_VERSION),
            extra: Map::new(),
        }
    }

    pub fn summary(&self) -> ChatSummary {
        let (phase, pending_question, pending_problem) = summary_state(&self.messages);
        ChatSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
            kind: normalize_kind(self.kind.as_deref()).to_owned(),
            message_count: self.messages.len(),
            phase,
            pending_question,
            pending_problem,
            settings: self.settings.clone(),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub kind: String,
    pub message_count: usize,
    pub phase: String,
    pub pending_question: bool,
    pub pending_problem: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<ChatSettings>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelSlot {
    #[serde(default)]
    pub model: Option<ModelRef>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelsSettings {
    #[serde(default)]
    pub strong: Option<ModelSlot>,
    #[serde(default)]
    pub cheap: Option<ModelSlot>,
    #[serde(default)]
    pub vision: Option<ModelSlot>,
}

#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentSettings {
    #[serde(default)]
    pub teaching_style: Option<String>,
    #[serde(default)]
    pub prompts: Option<Map<String, Value>>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    #[serde(default)]
    pub version: Option<u32>,
    #[serde(default)]
    pub models: Option<ModelsSettings>,
    #[serde(default)]
    pub agent: Option<AgentSettings>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl UserSettings {
    pub fn strong_model(&self) -> Option<&ModelRef> {
        self.models
            .as_ref()
            .and_then(|models| models.strong.as_ref())
            .and_then(|slot| slot.model.as_ref())
    }

    pub fn teaching_style(&self) -> &str {
        self.agent
            .as_ref()
            .and_then(|agent| agent.teaching_style.as_deref())
            .unwrap_or("guided")
    }
}

/// RFC 3339 UTC timestamp, matching the JavaScript `toISOString()` shape
/// used by LearnHarness chat records.
pub fn now_iso() -> String {
    let raw = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let Some(z) = raw.rfind('Z') else {
        return raw;
    };
    let (body, _) = raw.split_at(z);
    let (whole, fraction) = match body.find('.') {
        Some(dot) => (&body[..dot], Some(&body[dot + 1..])),
        None => (body, None),
    };
    let mut millis: String = fraction.unwrap_or_default().chars().take(3).collect();
    while millis.len() < 3 {
        millis.push('0');
    }
    format!("{whole}.{millis}Z")
}

pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_derives_kind_phase_and_pending_state() {
        let mut record = ChatRecord::new("chat_t".to_owned(), "T".to_owned());
        record.messages = vec![
            serde_json::json!({"role":"assistant","content":[{"type":"toolCall","id":"p1","name":"probe_done","arguments":{}}]}),
            serde_json::json!({"role":"toolResult","toolName":"probe_done","toolCallId":"p1","content":"ok"}),
            serde_json::json!({"role":"assistant","content":[{"type":"toolCall","id":"q1","name":"quiz","arguments":{"nodeId":"n1"}}]}),
        ];
        let summary = record.summary();
        assert_eq!(summary.kind, "teach");
        assert_eq!(summary.phase, "teach");
        assert!(summary.pending_question);
        assert!(!summary.pending_problem);
        record.kind = Some("practice".to_owned());
        assert_eq!(record.summary().kind, "practice");
    }

    #[test]
    fn iso_timestamps_use_javascript_millisecond_shape() {
        let now = now_iso();
        assert_eq!(now.len(), 24, "{now}");
        assert!(now.ends_with('Z'));
        assert_eq!(now.as_bytes()[19], b'.');
    }
}
