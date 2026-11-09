//! Digest-aware knowledge-graph extraction, merged into LearnHarness JSONL ledgers.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, anyhow};
use brainroom_agent::Completer;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Finding {
    #[serde(default)]
    pub propositions: Vec<Proposition>,
    #[serde(default)]
    pub chains: Vec<RawChain>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Proposition {
    #[serde(default, alias = "localId")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub statement: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default, rename = "messageId")]
    pub message_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RawChain {
    #[serde(default)]
    pub premises: Vec<String>,
    #[serde(default)]
    pub conclusion: String,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub nodes: usize,
    pub chains: usize,
    pub covered: usize,
}

pub trait Extractor: Send {
    fn extract(&self, prompt: &str) -> anyhow::Result<Finding>;
}

#[derive(Clone, Debug)]
struct Slot {
    message_id: String,
    lines: Vec<String>,
    fingerprint: String,
    tokens: usize,
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

fn hash_id(prefix: &str, seed: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    format!("{prefix}{}", &hex(hasher.finalize())[..16])
}

fn fingerprint(role: &str, lines: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(
        serde_json::to_string(&(role, lines))
            .unwrap_or_default()
            .as_bytes(),
    );
    hex(hasher.finalize())
}

fn now_iso() -> String {
    brainroom_core::now_iso()
}

fn text_parts(message: &Value) -> Vec<String> {
    match message.get("content") {
        Some(Value::String(text)) => vec![text.clone()],
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| {
                (p.get("type").and_then(Value::as_str) == Some("text"))
                    .then(|| p.get("text").and_then(Value::as_str).map(str::to_owned))
                    .flatten()
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn uncovered_slots(data_dir: &Path, chat_id: &str) -> Vec<Slot> {
    let path = data_dir.join("chats").join(format!("{chat_id}.json"));
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(record) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let covered: HashSet<String> = std::fs::read_to_string(data_dir.join("graph/coverage.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v.get("chatId").and_then(Value::as_str) == Some(chat_id))
        .filter_map(|v| v.get("fp").and_then(Value::as_str).map(str::to_owned))
        .collect();
    let mut out = Vec::new();
    if let Some(messages) = record.get("messages").and_then(Value::as_array) {
        for (i, message) in messages.iter().enumerate() {
            if message.get("role").and_then(Value::as_str) != Some("assistant") {
                continue;
            }
            let id = message
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("m{i}"));
            let lines: Vec<String> = text_parts(message)
                .into_iter()
                .filter(|t| !t.trim().is_empty())
                .map(|t| format!("TEACHER [{id}] {}", t.trim()))
                .collect();
            if lines.is_empty() {
                continue;
            }
            let fp = fingerprint("assistant", &lines);
            if covered.contains(&fp) {
                continue;
            }
            let tokens = lines.join("\n").len().div_ceil(4);
            out.push(Slot {
                message_id: id,
                lines,
                fingerprint: fp,
                tokens,
            });
        }
    }
    out
}

fn digest(data_dir: &Path, chat_id: &str) -> String {
    let nodes = std::fs::read_to_string(data_dir.join("graph/nodes.jsonl")).unwrap_or_default();
    let chains = std::fs::read_to_string(data_dir.join("graph/chains.jsonl")).unwrap_or_default();
    let ours: Vec<Value> = nodes
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| {
            v.get("provenance")
                .and_then(Value::as_array)
                .is_some_and(|p| {
                    p.iter()
                        .any(|x| x.get("chatId").and_then(Value::as_str) == Some(chat_id))
                })
        })
        .collect();
    let our_chains: Vec<Value> = chains
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|v| {
            v.get("provenance")
                .and_then(Value::as_array)
                .is_some_and(|p| {
                    p.iter()
                        .any(|x| x.get("chatId").and_then(Value::as_str) == Some(chat_id))
                })
        })
        .collect();
    if ours.is_empty() && our_chains.is_empty() {
        return String::new();
    }
    let mut out = String::from(
        "THE GRAPH SO FAR - everything already extracted from this source (reference these ids verbatim):\n",
    );
    for n in ours {
        out.push_str(&format!(
            "  {} | {} | {}\n",
            n.get("id").and_then(Value::as_str).unwrap_or_default(),
            n.get("name").and_then(Value::as_str).unwrap_or_default(),
            n.get("statement")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ));
    }
    if !our_chains.is_empty() {
        out.push_str("DERIVATIONS ALREADY RECORDED (build on these; do not re-derive them):\n");
        for c in our_chains {
            out.push_str(&format!(
                "  [{} => {}]\n",
                c.get("premises")
                    .and_then(Value::as_array)
                    .map(|p| p
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" + "))
                    .unwrap_or_default(),
                c.get("conclusion")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            ));
        }
    }
    out
}

fn merge(
    data_dir: &Path,
    chat_id: &str,
    finding: &Finding,
    slots: &[Slot],
) -> anyhow::Result<Stats> {
    let graph = data_dir.join("graph");
    std::fs::create_dir_all(&graph).context("create graph dir")?;
    let nodes_path = graph.join("nodes.jsonl");
    let chains_path = graph.join("chains.jsonl");
    let coverage_path = graph.join("coverage.jsonl");

    let existing_nodes: Vec<Value> = std::fs::read_to_string(&nodes_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let mut by_name: HashMap<String, String> = HashMap::new();
    let mut node_ids: HashSet<String> = HashSet::new();
    for n in &existing_nodes {
        if let (Some(name), Some(id)) = (
            n.get("name").and_then(Value::as_str),
            n.get("id").and_then(Value::as_str),
        ) {
            by_name
                .entry(name.to_lowercase())
                .or_insert_with(|| id.to_owned());
            node_ids.insert(id.to_owned());
        }
    }
    let valid_messages: HashSet<&str> = slots.iter().map(|s| s.message_id.as_str()).collect();
    let fallback_message = slots
        .first()
        .map(|s| s.message_id.clone())
        .unwrap_or_default();
    let mut new_nodes = String::new();
    let mut local: HashMap<String, String> = HashMap::new();
    let mut nodes_added = 0;
    for prop in &finding.propositions {
        if prop.name.trim().is_empty() || prop.statement.trim().is_empty() {
            continue;
        }
        let key = prop.name.trim().to_lowercase();
        let id = if let Some(id) = by_name.get(&key) {
            id.clone()
        } else {
            let id = hash_id("n_", &format!("{chat_id}:{}:{}", prop.name, prop.statement));
            by_name.insert(key, id.clone());
            node_ids.insert(id.clone());
            let message_id = if valid_messages.contains(prop.message_id.as_str()) {
                prop.message_id.clone()
            } else {
                fallback_message.clone()
            };
            new_nodes.push_str(
                &serde_json::to_string(&json!({
                    "id": id,
                    "name": prop.name,
                    "statement": prop.statement,
                    "aliases": prop.aliases,
                    "createdAt": now_iso(),
                    "provenance": [{ "chatId": chat_id, "messageId": message_id }],
                }))
                .unwrap_or_default(),
            );
            new_nodes.push('\n');
            nodes_added += 1;
            id
        };
        if !prop.id.is_empty() {
            local.insert(prop.id.clone(), id.clone());
        }
    }

    let existing_chains: Vec<Value> = std::fs::read_to_string(&chains_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let mut chain_keys: HashSet<String> = HashSet::new();
    for c in &existing_chains {
        let premises = c
            .get("premises")
            .and_then(Value::as_array)
            .map(|p| {
                let mut v: Vec<&str> = p.iter().filter_map(Value::as_str).collect();
                v.sort_unstable();
                v.join(",")
            })
            .unwrap_or_default();
        if let Some(conclusion) = c.get("conclusion").and_then(Value::as_str) {
            chain_keys.insert(format!("{premises}=>{conclusion}"));
        }
    }
    let resolve = |target: &str| -> Option<String> {
        local
            .get(target)
            .cloned()
            .or_else(|| node_ids.contains(target).then(|| target.to_owned()))
            .or_else(|| by_name.get(&target.to_lowercase()).cloned())
    };
    let mut new_chains = String::new();
    let mut chains_added = 0;
    for chain in &finding.chains {
        let Some(conclusion) = resolve(&chain.conclusion) else {
            continue;
        };
        let mut premises = Vec::new();
        let mut ok = true;
        for p in &chain.premises {
            match resolve(p) {
                Some(id) => premises.push(id),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok || premises.is_empty() {
            continue;
        }
        let mut sorted = premises.clone();
        sorted.sort_unstable();
        let key = format!("{}=>{}", sorted.join(","), conclusion);
        if !chain_keys.insert(key.clone()) {
            continue;
        }
        new_chains.push_str(
            &serde_json::to_string(&json!({
                "id": hash_id("arg_", &format!("{chat_id}:{key}")),
                "premises": premises,
                "conclusion": conclusion,
                "createdAt": now_iso(),
                "provenance": [{ "chatId": chat_id, "messageId": fallback_message }],
            }))
            .unwrap_or_default(),
        );
        new_chains.push('\n');
        chains_added += 1;
    }

    if !new_nodes.is_empty() {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&nodes_path)
            .context("open nodes.jsonl")?;
        file.write_all(new_nodes.as_bytes())?;
    }
    if !new_chains.is_empty() {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&chains_path)
            .context("open chains.jsonl")?;
        file.write_all(new_chains.as_bytes())?;
    }
    let mut coverage = String::new();
    for slot in slots {
        coverage.push_str(
            &serde_json::to_string(&json!({
                "chatId": chat_id,
                "fp": slot.fingerprint,
                "tokens": slot.tokens,
                "foldedAt": now_iso(),
            }))
            .unwrap_or_default(),
        );
        coverage.push('\n');
    }
    if !coverage.is_empty() {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&coverage_path)
            .context("open coverage.jsonl")?;
        file.write_all(coverage.as_bytes())?;
    }
    Ok(Stats {
        nodes: nodes_added,
        chains: chains_added,
        covered: slots.len(),
    })
}

/// Extract the uncovered teacher prose of one chat and merge it into the graph.
pub fn run(extractor: &dyn Extractor, data_dir: &Path, chat_id: &str) -> anyhow::Result<Stats> {
    let slots = uncovered_slots(data_dir, chat_id);
    if slots.is_empty() {
        return Ok(Stats::default());
    }
    let title = std::fs::read_to_string(data_dir.join("chats").join(format!("{chat_id}.json")))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.get("title").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default();
    let prose: Vec<String> = slots.iter().flat_map(|s| s.lines.clone()).collect();
    let mut prompt = String::new();
    if !title.is_empty() {
        prompt.push_str(&format!("# {title}\n\n"));
    }
    prompt.push_str(&prose.join("\n\n"));
    let digest = digest(data_dir, chat_id);
    if !digest.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&digest);
    }
    let finding = extractor.extract(&prompt)?;
    merge(data_dir, chat_id, &finding, &slots)
}

/// OpenRouter extraction using the `submit_findings` tool.
pub struct OpenRouterExtractor {
    pub model: String,
    pub timeout: Duration,
}

impl Extractor for OpenRouterExtractor {
    fn extract(&self, prompt: &str) -> anyhow::Result<Finding> {
        let completer = brainroom_agent::OpenRouter {
            model: self.model.clone(),
            timeout: self.timeout,
        };
        let tools = [brainroom_agent::ToolSpec {
            name: "submit_findings",
            description: "Submit the new propositions and reasoning chains this passage teaches.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "propositions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string", "description": "Local id like p1." },
                                "name": { "type": "string" },
                                "statement": { "type": "string" },
                                "aliases": { "type": "array", "items": { "type": "string" } },
                                "messageId": { "type": "string" }
                            },
                            "required": ["id", "name", "statement", "messageId"]
                        }
                    },
                    "chains": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "premises": { "type": "array", "items": { "type": "string" } },
                                "conclusion": { "type": "string" }
                            },
                            "required": ["premises", "conclusion"]
                        }
                    }
                },
                "required": ["propositions", "chains"]
            }),
        }];
        let messages = vec![json!({
            "role": "user",
            "content": format!(
                "You are the extraction stage of a learning application. Read the teacher's prose below \
                 and report the NEW core ideas and the reasoning chains that derive them. Each proposition \
                 must copy the message id from its passage's \"TEACHER [id]\" label into messageId. \
                 Submit with one submit_findings call.\n\n{prompt}"
            )
        })];
        let turn = completer.complete(&messages, &tools)?;
        let call = turn
            .tool_calls
            .iter()
            .find(|c| c.name == "submit_findings")
            .ok_or_else(|| anyhow!("extractor did not call submit_findings"))?;
        serde_json::from_value(call.arguments.clone()).context("parse submit_findings arguments")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fake {
        calls: AtomicUsize,
        finding: Finding,
    }

    impl Extractor for Fake {
        fn extract(&self, _: &str) -> anyhow::Result<Finding> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.finding.clone())
        }
    }

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "brainroom-extract-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("chats")).unwrap();
        std::fs::create_dir_all(dir.join("graph")).unwrap();
        dir
    }

    fn write_chat(dir: &Path, chat_id: &str) {
        let record = json!({
            "id": chat_id,
            "title": "Test lesson",
            "messages": [{
                "id": "m1",
                "role": "assistant",
                "content": [{ "type": "text", "text": "A model is a function of its training data." }]
            }]
        });
        std::fs::write(
            dir.join("chats").join(format!("{chat_id}.json")),
            record.to_string(),
        )
        .unwrap();
    }

    fn finding() -> Finding {
        Finding {
            propositions: vec![
                Proposition {
                    id: "p1".to_owned(),
                    name: "Model function".to_owned(),
                    statement: "A model's function is determined by its training data.".to_owned(),
                    aliases: vec![],
                    message_id: "m1".to_owned(),
                },
                Proposition {
                    id: "p2".to_owned(),
                    name: "Generalization".to_owned(),
                    statement: "Generalization is performance on unseen data.".to_owned(),
                    aliases: vec![],
                    message_id: "m1".to_owned(),
                },
            ],
            chains: vec![RawChain {
                premises: vec!["p1".to_owned()],
                conclusion: "p2".to_owned(),
            }],
        }
    }

    #[test]
    fn merges_nodes_and_chains_and_is_idempotent_by_coverage() {
        let dir = temp_dir();
        let chat = "chat_test";
        write_chat(&dir, chat);
        let extractor = Fake {
            calls: AtomicUsize::new(0),
            finding: finding(),
        };
        let first = run(&extractor, &dir, chat).unwrap();
        assert_eq!(first.nodes, 2);
        assert_eq!(first.chains, 1);
        assert_eq!(first.covered, 1);

        let nodes = std::fs::read_to_string(dir.join("graph/nodes.jsonl")).unwrap();
        assert_eq!(nodes.lines().count(), 2);
        let chains = std::fs::read_to_string(dir.join("graph/chains.jsonl")).unwrap();
        assert_eq!(chains.lines().count(), 1);
        let coverage = std::fs::read_to_string(dir.join("graph/coverage.jsonl")).unwrap();
        assert_eq!(coverage.lines().count(), 1);

        let second = run(&extractor, &dir, chat).unwrap();
        assert_eq!(second.nodes, 0);
        assert_eq!(second.chains, 0);
        assert_eq!(extractor.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    #[ignore = "needs OPENROUTER_API_KEY and network"]
    fn openrouter_extracts() {
        let dir = temp_dir();
        let chat = "chat_live";
        write_chat(&dir, chat);
        let model =
            brainroom_ai::model_id(&brainroom_ai::home_dir().join(".config/LearnHarness/data"));
        let extractor = OpenRouterExtractor {
            model,
            timeout: Duration::from_secs(120),
        };
        let stats = run(&extractor, &dir, chat).unwrap();
        assert!(stats.nodes > 0, "the extractor produced propositions");
    }
}
