//! Digest-aware knowledge-graph extraction, merged into LearnHarness JSONL ledgers.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use anyhow::{Context, anyhow};
use brainroom_agent::{Completer, Usage};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Default, serde::Serialize, Deserialize)]
pub struct Finding {
    #[serde(default)]
    pub propositions: Vec<Proposition>,
    #[serde(default)]
    pub chains: Vec<RawChain>,
}

#[derive(Clone, Debug, serde::Serialize, Deserialize)]
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

#[derive(Clone, Debug, serde::Serialize, Deserialize)]
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
    fn extract_with_usage(&self, prompt: &str) -> anyhow::Result<(Finding, Option<Usage>)> {
        Ok((self.extract(prompt)?, None))
    }
    fn model(&self) -> Option<&str> {
        None
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Link {
    #[serde(default, alias = "id", alias = "localId")]
    pub local_id: String,
    #[serde(default, alias = "linkTo", alias = "nodeId")]
    pub link_to: Option<String>,
}

pub trait Auditor: Send {
    fn audit(&self, propositions: &str, graph: &str) -> anyhow::Result<(Vec<Link>, Option<Usage>)>;
    fn model(&self) -> Option<&str> {
        None
    }
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
    format!("{prefix}{}", &hex(hasher.finalize())[..12])
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
    links: &[Link],
) -> anyhow::Result<Stats> {
    let graph = data_dir.join("graph");
    std::fs::create_dir_all(&graph).context("create graph dir")?;
    let nodes_path = graph.join("nodes.jsonl");
    let chains_path = graph.join("chains.jsonl");
    let coverage_path = graph.join("coverage.jsonl");

    let mut nodes: Vec<Value> = std::fs::read_to_string(&nodes_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let mut id_index: HashMap<String, usize> = HashMap::new();
    let mut by_name: HashMap<String, String> = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let Some(id) = node.get("id").and_then(Value::as_str) else {
            continue;
        };
        id_index.insert(id.to_owned(), index);
        if let Some(name) = node.get("name").and_then(Value::as_str) {
            by_name
                .entry(name.to_lowercase())
                .or_insert_with(|| id.to_owned());
        }
        if let Some(aliases) = node.get("aliases").and_then(Value::as_array) {
            for alias in aliases.iter().filter_map(Value::as_str) {
                by_name
                    .entry(alias.to_lowercase())
                    .or_insert_with(|| id.to_owned());
            }
        }
    }
    let linked: HashMap<&str, &str> = links
        .iter()
        .filter_map(|link| Some((link.local_id.as_str(), link.link_to.as_deref()?)))
        .collect();
    let valid_messages: HashSet<&str> = slots.iter().map(|s| s.message_id.as_str()).collect();
    let fallback_message = slots
        .first()
        .map(|s| s.message_id.clone())
        .unwrap_or_default();
    let mut local: HashMap<String, String> = HashMap::new();
    let mut nodes_added = 0;
    for prop in &finding.propositions {
        if prop.name.trim().is_empty() || prop.statement.trim().is_empty() {
            continue;
        }
        let key = prop.name.trim().to_lowercase();
        let message_id = if valid_messages.contains(prop.message_id.as_str()) {
            prop.message_id.clone()
        } else {
            fallback_message.clone()
        };
        let existing = linked
            .get(prop.id.as_str())
            .and_then(|id| id_index.get(*id).copied())
            .or_else(|| by_name.get(&key).and_then(|id| id_index.get(id).copied()));
        let id = if let Some(index) = existing {
            let node = &mut nodes[index];
            let id = node
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let mut aliases: Vec<Value> = node
                .get("aliases")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for alias in &prop.aliases {
                if !aliases.iter().any(|a| a.as_str() == Some(alias.as_str())) {
                    aliases.push(Value::String(alias.clone()));
                }
            }
            if !aliases.is_empty() {
                node["aliases"] = Value::Array(aliases);
            }
            let mut points = node
                .get("provenance")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let point = json!({ "chatId": chat_id, "messageId": message_id });
            if !points.iter().any(|p| {
                p.get("chatId").and_then(Value::as_str) == Some(chat_id)
                    && p.get("messageId").and_then(Value::as_str) == Some(message_id.as_str())
            }) {
                points.push(point);
                node["provenance"] = Value::Array(points);
            }
            id
        } else {
            let id = hash_id("n_", &format!("{chat_id}:{}:{}", prop.name, prop.statement));
            let node = json!({
                "id": id,
                "name": prop.name,
                "statement": prop.statement,
                "aliases": prop.aliases,
                "createdAt": now_iso(),
                "provenance": [{ "chatId": chat_id, "messageId": message_id }],
            });
            by_name.insert(key, id.clone());
            id_index.insert(id.clone(), nodes.len());
            nodes.push(node);
            nodes_added += 1;
            id
        };
        if !prop.id.is_empty() {
            local.insert(prop.id.clone(), id.clone());
        }
        if !local.contains_key(&prop.id) {
            local.insert(prop.id.clone(), id.clone());
        }
    }

    let mut chains: Vec<Value> = std::fs::read_to_string(&chains_path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect();
    let mut chain_keys: HashSet<String> = HashSet::new();
    for chain in &chains {
        let premises = chain
            .get("premises")
            .and_then(Value::as_array)
            .map(|p| {
                let mut v: Vec<&str> = p.iter().filter_map(Value::as_str).collect();
                v.sort_unstable();
                v.join(",")
            })
            .unwrap_or_default();
        if let Some(conclusion) = chain.get("conclusion").and_then(Value::as_str) {
            chain_keys.insert(format!("{premises}=>{conclusion}"));
        }
    }
    let node_ids: HashSet<String> = id_index.keys().cloned().collect();
    let resolve = |target: &str| -> Option<String> {
        local
            .get(target)
            .cloned()
            .or_else(|| node_ids.contains(target).then(|| target.to_owned()))
            .or_else(|| by_name.get(&target.to_lowercase()).cloned())
    };
    let mut chains_added = 0;
    for chain in &finding.chains {
        let Some(conclusion) = resolve(&chain.conclusion) else {
            continue;
        };
        let mut premises = Vec::new();
        let mut ok = true;
        for p in &chain.premises {
            match resolve(p) {
                Some(id) if !premises.contains(&id) => premises.push(id),
                Some(_) => {}
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
        chains.push(json!({
            "id": hash_id("arg_", &format!("{chat_id}:{key}")),
            "premises": premises,
            "conclusion": conclusion,
            "createdAt": now_iso(),
            "provenance": [{ "chatId": chat_id, "messageId": fallback_message }],
        }));
        chains_added += 1;
    }

    // Ancestry cover / prefer-fewest-premises: a chain whose premise set is a
    // strict superset of another chain with the same conclusion is redundant.
    let mut keep = vec![true; chains.len()];
    for i in 0..chains.len() {
        let Some(conclusion_i) = chains[i].get("conclusion").and_then(Value::as_str) else {
            continue;
        };
        let premises_i: HashSet<&str> = chains[i]
            .get("premises")
            .and_then(Value::as_array)
            .map(|p| p.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        for (j, chain_j) in chains.iter().enumerate() {
            if i == j || !keep[j] {
                continue;
            }
            if chain_j.get("conclusion").and_then(Value::as_str) != Some(conclusion_i) {
                continue;
            }
            let premises_j: HashSet<&str> = chain_j
                .get("premises")
                .and_then(Value::as_array)
                .map(|p| p.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            if premises_j.len() < premises_i.len() && premises_j.is_subset(&premises_i) {
                keep[i] = false;
                break;
            }
        }
    }
    let chains: Vec<Value> = chains
        .into_iter()
        .zip(keep)
        .filter_map(|(chain, keep)| keep.then_some(chain))
        .collect();

    write_lines(&nodes_path, &nodes)?;
    write_lines(&chains_path, &chains)?;
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

fn write_lines(path: &Path, values: &[Value]) -> anyhow::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut file = std::fs::File::create(&tmp)?;
        for value in values {
            writeln!(file, "{}", serde_json::to_string(value)?)?;
        }
    }
    std::fs::rename(tmp, path)?;
    Ok(())
}

fn graph_context(data_dir: &Path) -> String {
    let nodes = std::fs::read_to_string(data_dir.join("graph/nodes.jsonl")).unwrap_or_default();
    let mut lines = Vec::new();
    for line in nodes.lines().take(400) {
        let Ok(node) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let id = node.get("id").and_then(Value::as_str).unwrap_or_default();
        let name = node.get("name").and_then(Value::as_str).unwrap_or_default();
        let statement = node
            .get("statement")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let aliases = node
            .get("aliases")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .unwrap_or_default();
        lines.push(format!("{id} :: {name} :: {statement} :: {aliases}"));
    }
    lines.join("\n")
}

/// Extract the uncovered teacher prose of one chat and merge it into the graph.
pub fn run(extractor: &dyn Extractor, data_dir: &Path, chat_id: &str) -> anyhow::Result<Stats> {
    run_with_auditor(extractor, None, data_dir, chat_id)
}

pub fn run_with_auditor(
    extractor: &dyn Extractor,
    auditor: Option<&dyn Auditor>,
    data_dir: &Path,
    chat_id: &str,
) -> anyhow::Result<Stats> {
    let started = std::time::Instant::now();
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
    let (finding, extractor_usage) = extractor.extract_with_usage(&prompt)?;
    let propositions = serde_json::to_string(&finding.propositions)?;
    let graph = graph_context(data_dir);
    let (links, auditor_usage) = match auditor {
        Some(auditor) => auditor.audit(&propositions, &graph)?,
        None => (Vec::new(), None),
    };
    let stats = merge(data_dir, chat_id, &finding, &slots, &links)?;
    let _ = log_chat_usage(
        data_dir,
        chat_id,
        started.elapsed().as_millis() as u64,
        &stats,
        extractor.model(),
        extractor_usage,
        auditor.and_then(|a| a.model()),
        auditor_usage,
    );
    Ok(stats)
}

pub fn run_notes(
    extractor: &dyn Extractor,
    auditor: Option<&dyn Auditor>,
    data_dir: &Path,
    path: &Path,
) -> anyhow::Result<Stats> {
    let started = std::time::Instant::now();
    let text = std::fs::read_to_string(path).context("read notes file")?;
    let source_id = hash_id("src_", &path.to_string_lossy());
    let prompt = format!("# {}\n\n{text}", path.display());
    let (finding, extractor_usage) = extractor.extract_with_usage(&prompt)?;
    let propositions = serde_json::to_string(&finding.propositions)?;
    let graph = graph_context(data_dir);
    let (links, auditor_usage) = match auditor {
        Some(auditor) => auditor.audit(&propositions, &graph)?,
        None => (Vec::new(), None),
    };
    let stats = merge(data_dir, source_id.as_str(), &finding, &[], &links)?;
    let sources_path = data_dir.join("graph/sources.json");
    let mut sources: Value = std::fs::read_to_string(&sources_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}));
    let usage = json!({
        "startedAt": brainroom_core::now_millis(),
        "at": brainroom_core::now_iso(),
        "durationMs": started.elapsed().as_millis() as u64,
        "stats": { "nodes": stats.nodes, "chains": stats.chains },
        "stages": {
            "extractor": { "model": extractor.model().unwrap_or(""), "steps": [usage_json(extractor_usage)] },
            "auditor": { "model": auditor.and_then(|a| a.model()).unwrap_or(""), "steps": [usage_json(auditor_usage)] },
        }
    });
    if let Some(map) = sources.as_object_mut() {
        let entry = map.entry(source_id.clone()).or_insert_with(|| {
            json!({
                "id": source_id,
                "filePath": path.to_string_lossy(),
                "importedAt": brainroom_core::now_iso(),
            })
        });
        if let Some(object) = entry.as_object_mut() {
            object.insert("importedAt".to_owned(), json!(brainroom_core::now_iso()));
            let mut log = object
                .get("extractions")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            log.push(usage);
            object.insert("extractions".to_owned(), Value::Array(log));
        }
    }
    std::fs::write(sources_path, serde_json::to_string_pretty(&sources)?)?;
    Ok(stats)
}

fn usage_json(usage: Option<Usage>) -> Value {
    match usage {
        Some(usage) => json!({
            "inputTokens": usage.prompt_tokens,
            "outputTokens": usage.completion_tokens,
            "reasoningTokens": 0,
            "cacheReadTokens": 0,
            "cacheWriteTokens": 0,
        }),
        None => Value::Null,
    }
}

#[allow(clippy::too_many_arguments)]
fn log_chat_usage(
    data_dir: &Path,
    chat_id: &str,
    duration_ms: u64,
    stats: &Stats,
    extractor_model: Option<&str>,
    extractor_usage: Option<Usage>,
    auditor_model: Option<&str>,
    auditor_usage: Option<Usage>,
) -> anyhow::Result<()> {
    let store = brainroom_store::Store::new(data_dir.to_path_buf());
    let Some(mut record) = store.load_chat(chat_id) else {
        return Ok(());
    };
    let started_at = brainroom_core::now_millis();
    let entry = json!({
        "startedAt": started_at,
        "at": brainroom_core::now_iso(),
        "durationMs": duration_ms,
        "stats": { "nodes": stats.nodes, "chains": stats.chains },
        "stages": {
            "extractor": { "model": extractor_model.unwrap_or(""), "steps": [usage_json(extractor_usage)] },
            "auditor": { "model": auditor_model.unwrap_or(""), "steps": [usage_json(auditor_usage)] },
        }
    });
    let mut log = record
        .extra
        .get("extractions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    log.push(entry);
    record
        .extra
        .insert("extractions".to_owned(), Value::Array(log));
    store.save_chat(&record);
    Ok(())
}

fn extractor_tools() -> Vec<brainroom_agent::ToolSpec> {
    vec![brainroom_agent::ToolSpec {
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
    }]
}

fn extractor_prompt(prompt: &str) -> String {
    format!(
        "You are the extraction stage of a learning application. Read the teacher's prose below \
         and report the NEW core ideas and the reasoning chains that derive them. Each proposition \
         must copy the message id from its passage's \"TEACHER [id]\" label into messageId. \
         Submit with one submit_findings call.\n\n{prompt}"
    )
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
            thinking_level: None,
        };
        let tools = extractor_tools();
        let messages = vec![json!({ "role": "user", "content": extractor_prompt(prompt) })];
        let turn = completer.complete(&messages, &tools)?;
        let call = turn
            .tool_calls
            .iter()
            .find(|c| c.name == "submit_findings")
            .ok_or_else(|| anyhow!("extractor did not call submit_findings"))?;
        serde_json::from_value(call.arguments.clone()).context("parse submit_findings arguments")
    }

    fn extract_with_usage(&self, prompt: &str) -> anyhow::Result<(Finding, Option<Usage>)> {
        let completer = brainroom_agent::OpenRouter {
            model: self.model.clone(),
            timeout: self.timeout,
            thinking_level: None,
        };
        let tools = extractor_tools();
        let messages = vec![json!({
            "role": "user",
            "content": extractor_prompt(prompt)
        })];
        let turn = completer.complete(&messages, &tools)?;
        let call = turn
            .tool_calls
            .iter()
            .find(|c| c.name == "submit_findings")
            .ok_or_else(|| anyhow!("extractor did not call submit_findings"))?;
        let finding = serde_json::from_value(call.arguments.clone())
            .context("parse submit_findings arguments")?;
        Ok((finding, turn.usage))
    }

    fn model(&self) -> Option<&str> {
        Some(&self.model)
    }
}

pub struct OpenRouterAuditor {
    pub model: String,
    pub timeout: Duration,
}

impl Auditor for OpenRouterAuditor {
    fn audit(&self, propositions: &str, graph: &str) -> anyhow::Result<(Vec<Link>, Option<Usage>)> {
        let completer = brainroom_agent::OpenRouter {
            model: self.model.clone(),
            timeout: self.timeout,
            thinking_level: None,
        };
        let tools = [brainroom_agent::ToolSpec {
            name: "submit_links",
            description: "For each new proposition, link to an EXISTING graph id when it is the same idea, or omit linkTo to create a new node.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "links": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "localId": { "type": "string" },
                                "linkTo": { "type": ["string", "null"] }
                            },
                            "required": ["localId"]
                        }
                    }
                },
                "required": ["links"]
            }),
        }];
        let messages = vec![json!({
            "role": "user",
            "content": format!(
                "You are the identity auditor of a knowledge graph. For each NEW proposition below, decide whether it IS an existing graph node (same idea, even if phrased differently) and linkTo that node id; otherwise leave linkTo null so it is created. Never mint edges. Submit one submit_links call.\n\nNEW PROPOSITIONS:\n{propositions}\n\nEXISTING GRAPH (id :: name :: statement :: aliases):\n{graph}"
            )
        })];
        let turn = completer.complete(&messages, &tools)?;
        let call = turn
            .tool_calls
            .iter()
            .find(|c| c.name == "submit_links")
            .ok_or_else(|| anyhow!("auditor did not call submit_links"))?;
        let links: Vec<Link> = call
            .arguments
            .get("links")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .context("parse submit_links arguments")?
            .unwrap_or_default();
        Ok((links, turn.usage))
    }

    fn model(&self) -> Option<&str> {
        Some(&self.model)
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
            "createdAt": "2026-01-01T00:00:00.000Z",
            "updatedAt": "2026-01-01T00:00:00.000Z",
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

    fn slot(id: &str, fp: &str) -> Slot {
        Slot {
            message_id: id.to_owned(),
            lines: vec![format!("TEACHER [{id}] text")],
            fingerprint: fp.to_owned(),
            tokens: 1,
        }
    }

    #[test]
    fn auditor_links_merge_aliases_and_union_provenance() {
        let dir = temp_dir();
        let first = Finding {
            propositions: vec![Proposition {
                id: "p1".to_owned(),
                name: "Model function".to_owned(),
                statement: "A model is determined by its training data.".to_owned(),
                aliases: vec!["training-only".to_owned()],
                message_id: "m1".to_owned(),
            }],
            chains: Vec::new(),
        };
        merge(&dir, "chat_a", &first, &[slot("m1", "f1")], &[]).unwrap();
        let node_id = serde_json::from_str::<Value>(
            std::fs::read_to_string(dir.join("graph/nodes.jsonl"))
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let second = Finding {
            propositions: vec![Proposition {
                id: "p1".to_owned(),
                name: "Function of data".to_owned(),
                statement: "A model's behavior depends only on training data.".to_owned(),
                aliases: Vec::new(),
                message_id: "m1".to_owned(),
            }],
            chains: Vec::new(),
        };
        let links = vec![Link {
            local_id: "p1".to_owned(),
            link_to: Some(node_id.clone()),
        }];
        merge(&dir, "chat_b", &second, &[slot("m1", "f2")], &links).unwrap();
        let lines = std::fs::read_to_string(dir.join("graph/nodes.jsonl")).unwrap();
        assert_eq!(lines.lines().count(), 1);
        let node: Value = serde_json::from_str(lines.lines().next().unwrap()).unwrap();
        assert_eq!(node["provenance"].as_array().unwrap().len(), 2);
        assert!(
            node["aliases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "training-only")
        );
    }

    #[test]
    fn notes_extraction_uses_source_provenance() {
        let dir = temp_dir();
        let path = dir.join("notes.md");
        std::fs::write(&path, "A model is a function of its training data.").unwrap();
        let extractor = Fake {
            calls: AtomicUsize::new(0),
            finding: finding(),
        };
        let stats = run_notes(&extractor, None, &dir, &path).unwrap();
        assert_eq!(stats.nodes, 2);
        let sources: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("graph/sources.json")).unwrap())
                .unwrap();
        let (id, source) = sources.as_object().unwrap().iter().next().unwrap();
        assert!(id.starts_with("src_"));
        assert_eq!(source["filePath"], path.to_string_lossy().as_ref());
        let nodes = std::fs::read_to_string(dir.join("graph/nodes.jsonl")).unwrap();
        assert!(nodes.contains(id));
    }

    #[test]
    fn ancestry_cover_drops_superset_chains() {
        let dir = temp_dir();
        let finding = Finding {
            propositions: vec![
                Proposition {
                    id: "p1".to_owned(),
                    name: "A".to_owned(),
                    statement: "A".to_owned(),
                    aliases: Vec::new(),
                    message_id: "m1".to_owned(),
                },
                Proposition {
                    id: "p2".to_owned(),
                    name: "B".to_owned(),
                    statement: "B".to_owned(),
                    aliases: Vec::new(),
                    message_id: "m1".to_owned(),
                },
                Proposition {
                    id: "p3".to_owned(),
                    name: "C".to_owned(),
                    statement: "C".to_owned(),
                    aliases: Vec::new(),
                    message_id: "m1".to_owned(),
                },
            ],
            chains: vec![
                RawChain {
                    premises: vec!["p1".to_owned()],
                    conclusion: "p2".to_owned(),
                },
                RawChain {
                    premises: vec!["p1".to_owned(), "p3".to_owned()],
                    conclusion: "p2".to_owned(),
                },
            ],
        };
        let stats = merge(&dir, "chat", &finding, &[slot("m1", "f")], &[]).unwrap();
        assert_eq!(stats.chains, 2);
        let chains = std::fs::read_to_string(dir.join("graph/chains.jsonl")).unwrap();
        assert_eq!(chains.lines().count(), 1);
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
        let first_node: Value = serde_json::from_str(nodes.lines().next().unwrap()).unwrap();
        assert_eq!(first_node["id"].as_str().unwrap().len(), 14);
        let chains = std::fs::read_to_string(dir.join("graph/chains.jsonl")).unwrap();
        assert_eq!(chains.lines().count(), 1);
        let first_chain: Value = serde_json::from_str(chains.lines().next().unwrap()).unwrap();
        assert_eq!(first_chain["id"].as_str().unwrap().len(), 16);
        let coverage = std::fs::read_to_string(dir.join("graph/coverage.jsonl")).unwrap();
        assert_eq!(coverage.lines().count(), 1);

        let second = run(&extractor, &dir, chat).unwrap();
        assert_eq!(second.nodes, 0);
        assert_eq!(second.chains, 0);
        assert_eq!(extractor.calls.load(Ordering::SeqCst), 1);
        let record = brainroom_store::Store::new(dir.clone())
            .load_chat(chat)
            .unwrap();
        assert_eq!(
            record
                .extra
                .get("extractions")
                .and_then(Value::as_array)
                .unwrap()
                .len(),
            1
        );
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
