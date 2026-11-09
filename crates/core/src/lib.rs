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
        ChatSummary {
            id: self.id.clone(),
            title: self.title.clone(),
            updated_at: self.updated_at.clone(),
            message_count: self.messages.len(),
        }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub updated_at: String,
    pub message_count: usize,
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
    fn iso_timestamps_use_javascript_millisecond_shape() {
        let now = now_iso();
        assert_eq!(now.len(), 24, "{now}");
        assert!(now.ends_with('Z'));
        assert_eq!(now.as_bytes()[19], b'.');
    }
}
