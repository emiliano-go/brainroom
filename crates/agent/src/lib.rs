//! The teaching agent: an OpenAI-style tool loop that stops for learner answers.

mod prompts;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use anyhow::{Context, anyhow};
use serde_json::{Value, json};

pub type Message = Value;

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub cost: Option<f64>,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ThinkingBlock {
    pub text: String,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct AssistantTurn {
    pub text: String,
    pub thinking: Vec<ThinkingBlock>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
    pub api: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub raw_stop_reason: Option<String>,
    pub response_id: Option<String>,
    pub duration_ms: u64,
    pub thinking_level: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
}

#[derive(Clone, Debug)]
pub enum Event {
    Text(String),
    Quiz {
        call_id: String,
        tool: String,
        question: String,
        options: Vec<String>,
        values: Vec<String>,
        correct: Vec<usize>,
        multi_select: bool,
        graded: bool,
        text_mode: bool,
        question_type: Option<String>,
        node_id: Option<String>,
        details: Option<String>,
        committed: Option<String>,
        explanation: Option<String>,
    },
    Plan {
        mermaid: String,
    },
    Problem {
        call_id: String,
        statement: String,
        mode: String,
        solution: String,
    },
    Concluded {
        outcome: String,
        verdict: String,
        solution: String,
    },
    Assistant(Value),
    ToolResult {
        call_id: String,
        name: String,
        content: String,
        details: Option<Value>,
        is_error: bool,
        duration_ms: u64,
    },
    Usage(Usage),
    Done,
    Error(String),
}

#[derive(Clone, Debug)]
pub enum Input {
    User(String),
    ToolResult { call_id: String, content: String },
}

pub trait Completer: Send {
    fn complete(&self, messages: &[Message], tools: &[ToolSpec]) -> anyhow::Result<AssistantTurn>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Prober,
    Teach,
    Practice,
}

pub fn tools() -> Vec<ToolSpec> {
    tools_for(Mode::Teach)
}

pub fn tools_for(mode: Mode) -> Vec<ToolSpec> {
    match mode {
        Mode::Prober => prober_tools(),
        Mode::Teach => teach_tools(),
        Mode::Practice => practice_tools(),
    }
}

fn teach_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "quiz",
            description: "Ask one GRADED multiple-choice question. Supply correctAnswer as the correct option's value (or label), or an array for multiSelect. Include option values when labels are not unique. Call once, then end your turn.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "explanation": { "type": "string", "description": "Why the correct answer is correct." },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": { "type": "string" },
                                "value": { "type": "string" }
                            },
                            "required": ["label"]
                        }
                    },
                    "type": { "type": "string", "enum": ["check", "predict"], "description": "The question's role in the teaching cycle." },
                    "nodeId": { "type": "string", "description": "REQUIRED plan-tree node slug this question is about." },
                    "correctAnswer": {
                        "description": "The value (or label) of the correct option, or an array for multi-select.",
                        "type": ["string", "array"],
                        "items": { "type": "string" }
                    },
                    "multiSelect": { "type": "boolean" }
                },
                "required": ["type", "nodeId", "question", "options", "correctAnswer"]
            }),
        },
        ToolSpec {
            name: "ask_user_question",
            description: "Ask one UNGRADED question to collect a preference or decision. There is no correct answer. Options are labels with optional values/descriptions. Call once, then end your turn.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "details": { "type": "string" },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": { "type": "string" },
                                "value": { "type": "string" },
                                "description": { "type": "string" }
                            },
                            "required": ["label"]
                        }
                    }
                },
                "required": ["question"]
            }),
        },
        ToolSpec {
            name: "explain",
            description: "Ask one GRADED free-text question about a plan node. Commit the full correct answer in correctAnswer BEFORE posing; grade the reply against it next turn. ONE question per turn.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "enum": ["check", "predict"] },
                    "nodeId": { "type": "string", "description": "REQUIRED plan-tree node slug this question is about." },
                    "question": { "type": "string" },
                    "correctAnswer": { "type": "string" },
                    "details": { "type": "string" }
                },
                "required": ["type", "nodeId", "question", "correctAnswer"]
            }),
        },
        ToolSpec {
            name: "plan_init",
            description: "Record the teaching plan as a mermaid graph TD diagram.",
            parameters: json!({
                "type": "object",
                "properties": { "mermaid": { "type": "string" } },
                "required": ["mermaid"]
            }),
        },
        ToolSpec {
            name: "list_sessions",
            description: "List the sessions of the learner's persistent knowledge graph — every chat or imported-notes file that contributed a proposition, most recent first, with names, kinds, counts, dates and ids. Use the id with list_propositions.",
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "list_propositions",
            description: "List the propositions BORN in one knowledge-graph session — names only, no statement text. Pass the id exactly as list_sessions reported it.",
            parameters: json!({
                "type": "object",
                "properties": { "session": { "type": "string", "description": "Session id from list_sessions." } },
                "required": ["session"]
            }),
        },
        ToolSpec {
            name: "send_to_user",
            description: "Send a short message to the learner.",
            parameters: json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            }),
        },
    ]
}

fn prober_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "probe",
            description: "Ask one GRADED multiple-choice probe question, then end your turn. Probe calls have no type or nodeId. Supply correctAnswer as the correct option value (or label).",
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": { "label": { "type": "string" }, "value": { "type": "string" } },
                            "required": ["label"]
                        }
                    },
                    "correctAnswer": { "type": ["string", "array"], "items": { "type": "string" } },
                    "multiSelect": { "type": "boolean" },
                    "explanation": { "type": "string" }
                },
                "required": ["question", "options", "correctAnswer"]
            }),
        },
        ToolSpec {
            name: "recall",
            description: "Ask one GRADED free-text recall question. Commit the full correct answer in correctAnswer BEFORE posing. ONE question per turn.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "correctAnswer": { "type": "string" },
                    "details": { "type": "string" }
                },
                "required": ["question", "correctAnswer"]
            }),
        },
        ToolSpec {
            name: "ask_user_question",
            description: "Ask one UNGRADED question to collect a preference or decision.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "details": { "type": "string" },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": { "label": { "type": "string" }, "value": { "type": "string" }, "description": { "type": "string" } },
                            "required": ["label"]
                        }
                    }
                },
                "required": ["question"]
            }),
        },
        ToolSpec {
            name: "probe_done",
            description: "Conclude the probe phase with one entry per strand. Exactly one call; refused while a question is open.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "strands": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "floor": { "type": "string" },
                                "ceiling": { "type": "string" },
                                "misconceptions": { "type": "string" },
                                "buildFrom": { "type": "string" }
                            },
                            "required": ["name", "floor", "ceiling"]
                        }
                    }
                },
                "required": ["strands"]
            }),
        },
        ToolSpec {
            name: "list_sessions",
            description: "List the sessions of the learner's persistent knowledge graph — every chat or imported-notes file that contributed a proposition, most recent first, with names, kinds, counts, dates and ids. Use the id with list_propositions.",
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "list_propositions",
            description: "List the propositions born in one knowledge-graph session — names only.",
            parameters: json!({
                "type": "object",
                "properties": { "session": { "type": "string", "description": "Session id from list_sessions." } },
                "required": ["session"]
            }),
        },
    ]
}

fn practice_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "problem",
            description: "Pose one practice problem. Ends the turn; wait for the learner's answer.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "statement": { "type": "string" },
                    "mode": { "type": "string", "enum": ["recall", "discrimination", "composition"] },
                    "target": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional proposition ids/names exercised; omit for untagged problems."
                    },
                    "modelSolution": { "type": "string" }
                },
                "required": ["statement", "mode", "modelSolution"]
            }),
        },
        ToolSpec {
            name: "conclude",
            description: "End the active problem with a verdict and the reveal.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "outcome": { "type": "string", "enum": ["solved", "solvedWithHints", "movedOn", "surrendered", "dismissed"] },
                    "verdict": { "type": "string" },
                    "rung": { "type": ["string", "null"], "enum": ["R0", "R1", "R2", "R3", "R4", "R5", null] },
                    "missClass": { "type": ["string", "null"], "enum": ["cue", "discrimination", "execution", "careless", "structural", null] },
                    "retest": { "type": "boolean" }
                },
                "required": ["outcome", "verdict", "rung", "missClass", "retest"]
            }),
        },
        ToolSpec {
            name: "list_propositions",
            description: "List the propositions born in one knowledge-graph session — names only.",
            parameters: json!({
                "type": "object",
                "properties": { "session": { "type": "string", "description": "Session id from list_sessions." } },
                "required": ["session"]
            }),
        },
        ToolSpec {
            name: "list_sessions",
            description: "List the sessions of the learner's persistent knowledge graph — every chat or imported-notes file that contributed a proposition, most recent first, with names, kinds, counts, dates and ids. Use the id with list_propositions.",
            parameters: json!({ "type": "object", "properties": {} }),
        },
    ]
}

pub fn assistant_record(turn: &AssistantTurn) -> Value {
    let mut content = Vec::new();
    for thinking in &turn.thinking {
        let mut block = serde_json::Map::new();
        block.insert("type".to_owned(), json!("thinking"));
        block.insert("thinking".to_owned(), json!(thinking.text));
        if let Some(signature) = &thinking.signature {
            block.insert("thinkingSignature".to_owned(), json!(signature));
        }
        content.push(Value::Object(block));
    }
    if !turn.text.is_empty() {
        content.push(json!({ "type": "text", "text": turn.text }));
    }
    for call in &turn.tool_calls {
        content.push(json!({
            "type": "toolCall",
            "id": call.id,
            "name": call.name,
            "arguments": call.arguments,
        }));
    }
    let mut message = serde_json::Map::new();
    message.insert("role".to_owned(), json!("assistant"));
    message.insert("content".to_owned(), json!(content));
    if let Some(api) = &turn.api {
        message.insert("api".to_owned(), json!(api));
    }
    if let Some(provider) = &turn.provider {
        message.insert("provider".to_owned(), json!(provider));
    }
    if let Some(model) = &turn.model {
        message.insert("model".to_owned(), json!(model));
    }
    if let Some(usage) = turn.usage {
        message.insert(
            "usage".to_owned(),
            json!({
                "input": usage.prompt_tokens,
                "output": usage.completion_tokens,
                "cacheRead": usage.cache_read_tokens,
                "cacheWrite": usage.cache_write_tokens,
                "reasoning": usage.reasoning_tokens,
                "totalTokens": usage.total_tokens,
                "cost": { "total": usage.cost.unwrap_or(0.0) },
            }),
        );
    }
    if let Some(stop) = &turn.stop_reason {
        message.insert("stopReason".to_owned(), json!(stop));
    }
    if let Some(raw) = &turn.raw_stop_reason {
        message.insert("rawStopReason".to_owned(), json!(raw));
    }
    if let Some(response) = &turn.response_id {
        message.insert("responseId".to_owned(), json!(response));
    }
    message.insert("durationMs".to_owned(), json!(turn.duration_ms));
    if let Some(level) = &turn.thinking_level {
        message.insert("thinkingLevel".to_owned(), json!(level));
    }
    Value::Object(message)
}

fn emit_tool_result(
    emit: &Sender<Event>,
    call_id: &str,
    name: &str,
    content: String,
    details: Option<Value>,
    is_error: bool,
    duration_ms: u64,
) {
    let _ = emit.send(Event::ToolResult {
        call_id: call_id.to_owned(),
        name: name.to_owned(),
        content,
        details,
        is_error,
        duration_ms,
    });
}

fn system_message(mode: Mode, data_dir: &Path) -> Message {
    let kind = match mode {
        Mode::Prober => prompts::AgentKind::Prober,
        Mode::Teach => prompts::AgentKind::Teacher,
        Mode::Practice => prompts::AgentKind::Practice,
    };
    json!({ "role": "system", "content": prompts::compose(kind, data_dir) })
}

pub fn user_message(text: &str) -> Message {
    json!({ "role": "user", "content": text })
}

fn assistant_message(turn: &AssistantTurn) -> Message {
    if turn.tool_calls.is_empty() {
        return json!({ "role": "assistant", "content": turn.text });
    }
    let calls: Vec<Value> = turn
        .tool_calls
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "type": "function",
                "function": { "name": c.name, "arguments": c.arguments.to_string() }
            })
        })
        .collect();
    let content = if turn.text.is_empty() {
        Value::Null
    } else {
        json!(turn.text)
    };
    json!({ "role": "assistant", "content": content, "tool_calls": calls })
}

fn tool_result(call_id: &str, content: &str) -> Message {
    json!({ "role": "tool", "tool_call_id": call_id, "content": content })
}

fn quiz_options(call: &ToolCall) -> (Vec<String>, Vec<String>) {
    let mut labels = Vec::new();
    let mut values = Vec::new();
    if let Some(opts) = call.arguments.get("options").and_then(Value::as_array) {
        for option in opts {
            let label = option
                .get("label")
                .and_then(Value::as_str)
                .or_else(|| option.as_str())
                .unwrap_or_default()
                .trim()
                .to_owned();
            let value = option
                .get("value")
                .and_then(Value::as_str)
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| label.clone());
            labels.push(label);
            values.push(value);
        }
    }
    (labels, values)
}

fn quiz_correct(call: &ToolCall, labels: &[String], values: &[String]) -> Vec<usize> {
    let wanted: Vec<String> = match call.arguments.get("correctAnswer") {
        Some(Value::String(answer)) => vec![answer.clone()],
        Some(Value::Array(answers)) => answers
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for answer in wanted {
        let answer = answer.trim();
        let index = values
            .iter()
            .position(|v| v == answer)
            .or_else(|| labels.iter().position(|l| l == answer));
        if let Some(index) = index
            && !out.contains(&index)
        {
            out.push(index);
        }
    }
    out.sort_unstable();
    out
}

fn quiz_question(call: &ToolCall) -> String {
    call.arguments
        .get("question")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn graph_tools_enabled(data_dir: &Path) -> bool {
    let settings = std::fs::read_to_string(data_dir.join("settings.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    let graph_on = settings
        .as_ref()
        .and_then(|s| s.pointer("/graph/enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let tools_on = settings
        .as_ref()
        .and_then(|s| s.pointer("/agent/graphTools/enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    graph_on && tools_on
}

fn tools_for_session(mode: Mode, graph_enabled: bool) -> Vec<ToolSpec> {
    let mut tools = tools_for(mode);
    if !graph_enabled {
        tools.retain(|tool| !matches!(tool.name, "list_sessions" | "list_propositions"));
    }
    tools
}

fn graph_nodes(data_dir: &Path) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    std::fs::read_to_string(data_dir.join("graph/nodes.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|node| {
            node.get("id")
                .and_then(Value::as_str)
                .map(|id| seen.insert(id.to_owned()))
                .unwrap_or(false)
        })
        .collect()
}

fn birth_session(node: &Value) -> Option<&str> {
    node.get("provenance")
        .and_then(Value::as_array)
        .and_then(|points| points.first())
        .and_then(|point| point.get("chatId"))
        .and_then(Value::as_str)
}

fn list_sessions(data_dir: &Path) -> String {
    let nodes = graph_nodes(data_dir);
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for node in &nodes {
        if let Some(birth) = birth_session(node) {
            *counts.entry(birth.to_owned()).or_default() += 1;
        }
    }
    if counts.is_empty() {
        return "The knowledge graph is empty — no session has contributed propositions yet."
            .to_owned();
    }
    let store = brainroom_store::Store::new(data_dir.to_path_buf());
    let mut entries: Vec<(String, String, String, String, usize)> = Vec::new();
    let sources: Value = std::fs::read_to_string(data_dir.join("graph/sources.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    for (id, propositions) in counts {
        if let Some(chat) = store.load_chat(&id) {
            entries.push((
                id,
                chat.title,
                "chat".to_owned(),
                format!("{}, last active {}", chat.created_at, chat.updated_at),
                propositions,
            ));
        } else {
            let source = sources.get(&id);
            let name = source
                .and_then(|s| s.get("filePath"))
                .and_then(Value::as_str)
                .unwrap_or(&id)
                .to_owned();
            let stamp = source
                .and_then(|s| s.get("importedAt"))
                .and_then(Value::as_str)
                .unwrap_or("unknown date")
                .to_owned();
            entries.push((id, name, "file".to_owned(), stamp, propositions));
        }
    }
    entries.sort_by(|a, b| b.3.cmp(&a.3));
    let lines: Vec<String> = entries
        .iter()
        .enumerate()
        .map(|(i, (id, name, kind, dates, propositions))| {
            let plural = if *propositions == 1 { "" } else { "s" };
            format!(
                "{}. {name} ({kind}, {propositions} proposition{plural}, born {dates}) — id: {id}",
                i + 1
            )
        })
        .collect();
    format!(
        "{} session(s) in the knowledge graph, most recent first:\n{}",
        entries.len(),
        lines.join("\n")
    )
}

fn list_propositions(data_dir: &Path, session: &str) -> String {
    let mut names: Vec<(String, String)> = graph_nodes(data_dir)
        .into_iter()
        .filter(|node| birth_session(node) == Some(session))
        .filter_map(|node| {
            Some((
                node.get("id").and_then(Value::as_str)?.to_owned(),
                node.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ))
        })
        .collect();
    if names.is_empty() {
        return format!(
            "No propositions born in session \"{session}\". Call list_sessions for the current ids."
        );
    }
    names.sort_by(|a, b| a.0.cmp(&b.0));
    let lines: Vec<String> = names.iter().map(|(_, name)| format!("- {name}")).collect();
    format!(
        "{} proposition(s) covered in this session:\n{}",
        names.len(),
        lines.join("\n")
    )
}

fn active_problem(messages: &[Message]) -> bool {
    let mut active = false;
    for message in messages {
        let Some(calls) = message.get("tool_calls").and_then(Value::as_array) else {
            continue;
        };
        for call in calls {
            match call.pointer("/function/name").and_then(Value::as_str) {
                Some("problem") => active = true,
                Some("conclude") => active = false,
                _ => {}
            }
        }
    }
    active
}

fn known_graph_targets(data_dir: &Path) -> Vec<(String, String)> {
    let path = data_dir.join("graph/nodes.jsonl");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|node| {
            Some((
                node.get("id").and_then(Value::as_str)?.to_owned(),
                node.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_lowercase(),
            ))
        })
        .collect()
}

struct MermaidPlan {
    nodes: Vec<(String, String)>,
    problems: Vec<String>,
}

fn endpoint(src: &str) -> (String, String) {
    let src = src.trim();
    if let Some(open) = src.find('[') {
        let id = src[..open].trim().to_owned();
        let title = src[open + 1..]
            .trim_end_matches(']')
            .trim()
            .trim_matches('"')
            .to_owned();
        (id, title)
    } else {
        (src.to_owned(), src.to_owned())
    }
}

fn parse_mermaid_plan(src: &str) -> MermaidPlan {
    let mut nodes: Vec<(String, String)> = Vec::new();
    let mut edges: Vec<(String, String)> = Vec::new();
    let mut problems = Vec::new();
    for line in src.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("%%")
            || line.starts_with("graph")
            || line.starts_with("flowchart")
            || line.starts_with("classDef")
            || line.starts_with("style")
            || line.starts_with("linkStyle")
        {
            continue;
        }
        let parts: Vec<&str> = line.split("-->").map(str::trim).collect();
        if parts.len() < 2 {
            continue;
        }
        for part in &parts {
            let (id, title) = endpoint(part);
            if id.is_empty() {
                continue;
            }
            if !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
            {
                problems.push(format!("node id \"{id}\" must be a lowercase slug"));
            }
            match nodes.iter_mut().find(|(existing, _)| existing == &id) {
                Some((_, existing_title)) => {
                    if existing_title != &title && title != id {
                        problems.push(format!("node id \"{id}\" is declared twice"));
                    }
                }
                None => nodes.push((id, title)),
            }
        }
        for window in parts.windows(2) {
            let (from, _) = endpoint(window[0]);
            let (to, _) = endpoint(window[1]);
            if from == to {
                problems.push(format!("self-loop on node \"{from}\""));
            }
            if !from.is_empty() && !to.is_empty() {
                edges.push((from, to));
            }
        }
    }
    if nodes.is_empty() {
        problems.push("plan has no nodes".to_owned());
    }
    let mut color: std::collections::HashMap<&str, u8> = std::collections::HashMap::new();
    fn visit<'a>(
        node: &'a str,
        edges: &'a [(String, String)],
        color: &mut std::collections::HashMap<&'a str, u8>,
        problems: &mut Vec<String>,
    ) {
        match color.get(node).copied().unwrap_or(0) {
            1 => {
                problems.push(format!("plan contains a cycle through \"{node}\""));
                return;
            }
            2 => return,
            _ => {}
        }
        color.insert(node, 1);
        for (from, to) in edges {
            if from == node {
                visit(to, edges, color, problems);
            }
        }
        color.insert(node, 2);
    }
    for (id, _) in &nodes {
        visit(id, &edges, &mut color, &mut problems);
    }
    MermaidPlan { nodes, problems }
}

fn mermaid_node_ids(mermaid: &str) -> Vec<String> {
    parse_mermaid_plan(mermaid)
        .nodes
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

fn existing_plan_nodes(messages: &[Message]) -> std::collections::HashSet<String> {
    let mut nodes = std::collections::HashSet::new();
    for message in messages {
        let Some(calls) = message.get("tool_calls").and_then(Value::as_array) else {
            continue;
        };
        for call in calls {
            if call.pointer("/function/name").and_then(Value::as_str) != Some("plan_init") {
                continue;
            }
            let raw = call
                .pointer("/function/arguments")
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let Ok(arguments) = serde_json::from_str::<Value>(raw) else {
                continue;
            };
            if let Some(mermaid) = arguments.get("mermaid").and_then(Value::as_str) {
                nodes.extend(mermaid_node_ids(mermaid));
            }
        }
    }
    nodes
}

/// Run the agent until its inbox closes. Blocking tools (`quiz`, `ask_user_question`)
/// emit an event and wait for the matching [`Input::ToolResult`].
pub fn run(
    completer: &dyn Completer,
    data_dir: PathBuf,
    initial: Vec<Message>,
    inbox: Receiver<Input>,
    emit: Sender<Event>,
) {
    run_with_mode(completer, data_dir, initial, inbox, emit, Mode::Teach);
}

pub fn run_with_mode(
    completer: &dyn Completer,
    data_dir: PathBuf,
    initial: Vec<Message>,
    inbox: Receiver<Input>,
    emit: Sender<Event>,
    mode: Mode,
) {
    let mut problem_solution: Option<String> = None;
    let mut messages = vec![system_message(mode, &data_dir)];
    let mut plan_nodes = existing_plan_nodes(&initial);
    let mut problem_active = active_problem(&initial);
    let session_tools = tools_for_session(mode, graph_tools_enabled(&data_dir));
    messages.extend(initial);
    while let Ok(first) = inbox.recv() {
        match first {
            Input::User(text) => messages.push(user_message(&text)),
            Input::ToolResult { call_id, content } => {
                messages.push(tool_result(&call_id, &content))
            }
        }
        let mut steps = 0;
        loop {
            steps += 1;
            if steps > 12 {
                let _ = emit.send(Event::Error("agent exceeded 12 tool steps".to_owned()));
                break;
            }
            let turn = match completer.complete(&messages, &session_tools) {
                Ok(turn) => turn,
                Err(error) => {
                    let _ = emit.send(Event::Error(error.to_string()));
                    break;
                }
            };
            if !turn.text.is_empty() {
                let _ = emit.send(Event::Text(turn.text.clone()));
            }
            if let Some(usage) = turn.usage {
                let _ = emit.send(Event::Usage(usage));
            }
            let _ = emit.send(Event::Assistant(assistant_record(&turn)));
            messages.push(assistant_message(&turn));
            if turn.tool_calls.is_empty() {
                let _ = emit.send(Event::Done);
                break;
            }
            let mut blocking: Option<(String, String)> = None;
            let mut question_posed = false;
            let mut terminate_turn = false;
            for call in &turn.tool_calls {
                match call.name.as_str() {
                    "problem" => {
                        if problem_active {
                            let content =
                                "a problem is already active — call conclude on it before posing the next"
                                    .to_owned();
                            messages.push(tool_result(&call.id, &content));
                            emit_tool_result(&emit, &call.id, &call.name, content, None, true, 0);
                            continue;
                        }
                        if let Some(targets) =
                            call.arguments.get("target").and_then(Value::as_array)
                            && !targets.is_empty()
                        {
                            let known = known_graph_targets(&data_dir);
                            let unknown: Vec<&str> = targets
                                .iter()
                                .filter_map(Value::as_str)
                                .filter(|target| {
                                    let lower = target.to_lowercase();
                                    !known
                                        .iter()
                                        .any(|(id, name)| id == target || *name == lower)
                                })
                                .collect();
                            if !unknown.is_empty() {
                                let content = format!(
                                    "target {unknown:?} is not a known proposition — use graph ids/names, or omit target entirely"
                                );
                                messages.push(tool_result(&call.id, &content));
                                emit_tool_result(
                                    &emit, &call.id, &call.name, content, None, true, 0,
                                );
                                continue;
                            }
                        }
                        let statement = call
                            .arguments
                            .get("statement")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let mode_name = call
                            .arguments
                            .get("mode")
                            .and_then(Value::as_str)
                            .unwrap_or("recall")
                            .to_owned();
                        let solution = call
                            .arguments
                            .get("modelSolution")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        problem_solution = Some(solution.clone());
                        problem_active = true;
                        let _ = emit.send(Event::Problem {
                            call_id: call.id.clone(),
                            statement,
                            mode: mode_name,
                            solution,
                        });
                        let content = "Problem posed — the turn ends here; the learner's thinking traces and answer commits arrive as labeled messages.".to_owned();
                        messages.push(tool_result(&call.id, &content));
                        emit_tool_result(&emit, &call.id, &call.name, content, None, false, 0);
                        terminate_turn = true;
                    }
                    "conclude" => {
                        if !problem_active {
                            let content =
                                "no active problem to conclude — pose one first".to_owned();
                            messages.push(tool_result(&call.id, &content));
                            emit_tool_result(&emit, &call.id, &call.name, content, None, true, 0);
                            continue;
                        }
                        problem_active = false;
                        let outcome = call
                            .arguments
                            .get("outcome")
                            .and_then(Value::as_str)
                            .unwrap_or("movedOn")
                            .to_owned();
                        let verdict = call
                            .arguments
                            .get("verdict")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let _ = emit.send(Event::Concluded {
                            outcome: outcome.clone(),
                            verdict: verdict.clone(),
                            solution: problem_solution.take().unwrap_or_default(),
                        });
                        let content =
                            "Problem concluded — the committed model solution is revealed."
                                .to_owned();
                        let details = json!({
                            "outcome": outcome,
                            "verdict": verdict,
                            "rung": call.arguments.get("rung").cloned().unwrap_or(Value::Null),
                            "missClass": call.arguments.get("missClass").cloned().unwrap_or(Value::Null),
                            "retest": call.arguments.get("retest").and_then(Value::as_bool).unwrap_or(false),
                        });
                        messages.push(tool_result(&call.id, &content));
                        emit_tool_result(
                            &emit,
                            &call.id,
                            &call.name,
                            content,
                            Some(details),
                            false,
                            0,
                        );
                    }
                    "quiz" | "probe" | "explain" | "recall" | "ask_user_question" => {
                        let node_id = call
                            .arguments
                            .get("nodeId")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim()
                            .to_owned();
                        if matches!(call.name.as_str(), "quiz" | "explain") {
                            if node_id.is_empty() {
                                let content = "nodeId is required for teacher quiz/explain calls — copy an exact plan_init slug.".to_owned();
                                messages.push(tool_result(&call.id, &content));
                                emit_tool_result(
                                    &emit, &call.id, &call.name, content, None, true, 0,
                                );
                                continue;
                            }
                            if !plan_nodes.contains(&node_id) {
                                let content = format!(
                                    "nodeId \"{node_id}\" is not in this session's plan — use an exact plan_init slug."
                                );
                                messages.push(tool_result(&call.id, &content));
                                emit_tool_result(
                                    &emit, &call.id, &call.name, content, None, true, 0,
                                );
                                continue;
                            }
                        }
                        if question_posed {
                            let content =
                                "Only one question may be posed per turn; this call was not shown."
                                    .to_owned();
                            messages.push(tool_result(&call.id, &content));
                            emit_tool_result(&emit, &call.id, &call.name, content, None, true, 0);
                            continue;
                        }
                        question_posed = true;
                        let question = quiz_question(call);
                        let (options, values) = quiz_options(call);
                        let correct = quiz_correct(call, &options, &values);
                        let multi_select = call
                            .arguments
                            .get("multiSelect")
                            .and_then(Value::as_bool)
                            .unwrap_or(correct.len() > 1);
                        let explanation = call
                            .arguments
                            .get("explanation")
                            .and_then(Value::as_str)
                            .map(str::to_owned);
                        let text_mode = matches!(call.name.as_str(), "explain" | "recall")
                            || (call.name == "ask_user_question" && options.is_empty());
                        let graded =
                            matches!(call.name.as_str(), "quiz" | "probe" | "explain" | "recall");
                        let _ = emit.send(Event::Quiz {
                            call_id: call.id.clone(),
                            tool: call.name.clone(),
                            graded,
                            text_mode,
                            question: question.clone(),
                            options,
                            values,
                            correct,
                            multi_select,
                            question_type: call
                                .arguments
                                .get("type")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            node_id: (!node_id.is_empty()).then_some(node_id),
                            details: call
                                .arguments
                                .get("details")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            committed: call
                                .arguments
                                .get("correctAnswer")
                                .and_then(Value::as_str)
                                .filter(|text| !text.trim().is_empty())
                                .map(str::to_owned),
                            explanation,
                        });
                        blocking = Some((call.id.clone(), question));
                    }
                    "plan_init" => {
                        let mermaid = call
                            .arguments
                            .get("mermaid")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let plan = parse_mermaid_plan(&mermaid);
                        if !plan.problems.is_empty() {
                            let content = plan.problems.join("\n");
                            messages.push(tool_result(&call.id, &content));
                            emit_tool_result(&emit, &call.id, &call.name, content, None, true, 0);
                            continue;
                        }
                        plan_nodes.extend(plan.nodes.iter().map(|(id, _)| id.clone()));
                        let content = json!({
                            "ok": true,
                            "nodeIds": plan.nodes.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
                            "reminder": "These are the plan's node ids — the only valid nodeId values for every quiz and explain call from here on. Reference them exactly."
                        })
                        .to_string();
                        let _ = emit.send(Event::Plan { mermaid });
                        messages.push(tool_result(&call.id, &content));
                        emit_tool_result(&emit, &call.id, &call.name, content, None, false, 0);
                    }
                    "probe_done" => {
                        let strands = call
                            .arguments
                            .get("strands")
                            .cloned()
                            .unwrap_or_else(|| json!([]));
                        let content = json!({ "ok": true, "strands": strands }).to_string();
                        messages.push(tool_result(&call.id, &content));
                        emit_tool_result(
                            &emit,
                            &call.id,
                            &call.name,
                            content,
                            Some(strands),
                            false,
                            0,
                        );
                    }
                    "list_sessions" => {
                        let content = list_sessions(&data_dir);
                        messages.push(tool_result(&call.id, &content));
                        emit_tool_result(&emit, &call.id, &call.name, content, None, false, 0);
                    }
                    "list_propositions" => {
                        let session = call
                            .arguments
                            .get("session")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let content = list_propositions(&data_dir, session);
                        messages.push(tool_result(&call.id, &content));
                        emit_tool_result(&emit, &call.id, &call.name, content, None, false, 0);
                    }
                    "send_to_user" => {
                        let text = call
                            .arguments
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let _ = emit.send(Event::Text(text));
                        messages.push(tool_result(&call.id, "Message received for display."));
                        emit_tool_result(
                            &emit,
                            &call.id,
                            &call.name,
                            "Message received for display.".to_owned(),
                            None,
                            false,
                            0,
                        );
                    }
                    other => {
                        let content = format!("Unknown tool: {other}");
                        messages.push(tool_result(&call.id, &content));
                        emit_tool_result(&emit, &call.id, &call.name, content, None, true, 0);
                    }
                }
            }
            if terminate_turn {
                let _ = emit.send(Event::Done);
                break;
            }
            if let Some((call_id, _)) = blocking {
                match inbox.recv() {
                    Ok(Input::ToolResult {
                        call_id: got,
                        content,
                    }) => {
                        let id = if got.is_empty() { call_id } else { got };
                        messages.push(tool_result(&id, &content));
                    }
                    Ok(Input::User(text)) => messages.push(user_message(&text)),
                    Err(_) => return,
                }
            }
        }
    }
}

/// OpenRouter chat completion with tool calling (streaming + retries).
pub struct OpenRouter {
    pub model: String,
    pub timeout: Duration,
    pub thinking_level: Option<String>,
}

fn usage_from_value(value: &Value) -> Option<Usage> {
    let usage = value;
    let prompt = usage
        .get("prompt_tokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion = usage
        .get("completion_tokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_read = usage
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning = usage
        .pointer("/completion_tokens_details/reasoning_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(prompt + completion);
    (total > 0 || prompt > 0 || completion > 0).then_some(Usage {
        prompt_tokens: prompt,
        completion_tokens: completion,
        total_tokens: total,
        cost: usage.get("cost").and_then(Value::as_f64),
        cache_read_tokens: cache_read,
        cache_write_tokens: 0,
        reasoning_tokens: reasoning,
    })
}

fn finalize_usage(usage: Option<Usage>, model: &str) -> Option<Usage> {
    usage.map(|mut usage| {
        if usage.cost.is_none() {
            usage.cost =
                brainroom_ai::estimated_cost(model, usage.prompt_tokens, usage.completion_tokens);
        }
        usage
    })
}

fn finish_reason(value: Option<&str>) -> Option<&'static str> {
    value.map(|reason| match reason {
        "tool_calls" => "toolUse",
        "length" => "length",
        "content_filter" => "error",
        _ => "stop",
    })
}

fn parse_tool_calls(calls: &[(String, String, String)]) -> Vec<ToolCall> {
    calls
        .iter()
        .map(|(id, name, raw)| {
            let arguments =
                serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.clone()));
            ToolCall {
                id: id.clone(),
                name: name.clone(),
                arguments,
            }
        })
        .collect()
}

fn parse_completion_json(
    text: &str,
    started: std::time::Instant,
    requested_model: &str,
) -> anyhow::Result<AssistantTurn> {
    let value: Value = serde_json::from_str(text).context("parse OpenRouter response")?;
    let message = value
        .pointer("/choices/0/message")
        .ok_or_else(|| anyhow!("OpenRouter response has no message: {text}"))?;
    let reply = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut calls = Vec::new();
    if let Some(list) = message.get("tool_calls").and_then(Value::as_array) {
        for call in list {
            calls.push((
                call.get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                call.pointer("/function/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                call.pointer("/function/arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}")
                    .to_owned(),
            ));
        }
    }
    let thinking = message
        .get("reasoning")
        .and_then(Value::as_str)
        .or_else(|| message.get("reasoning_content").and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .map(|text| ThinkingBlock {
            text: text.to_owned(),
            signature: message
                .get("reasoning_details")
                .filter(|details| !details.is_null())
                .map(Value::to_string),
        })
        .into_iter()
        .collect();
    let raw_finish = value
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(AssistantTurn {
        text: reply,
        thinking,
        tool_calls: parse_tool_calls(&calls),
        usage: finalize_usage(
            value.get("usage").and_then(usage_from_value),
            value
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or(requested_model),
        ),
        api: Some("openai-completions".to_owned()),
        provider: Some("openrouter".to_owned()),
        model: value
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| Some(requested_model.to_owned())),
        stop_reason: finish_reason(raw_finish.as_deref()).map(str::to_owned),
        raw_stop_reason: raw_finish,
        response_id: value.get("id").and_then(Value::as_str).map(str::to_owned),
        duration_ms: started.elapsed().as_millis() as u64,
        thinking_level: None,
    })
}

fn parse_completion_stream(
    response: reqwest::blocking::Response,
    started: std::time::Instant,
    requested_model: &str,
) -> anyhow::Result<AssistantTurn> {
    use std::io::BufRead;
    let mut reader = std::io::BufReader::new(response);
    let mut line = String::new();
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut calls: std::collections::BTreeMap<usize, (String, String, String)> =
        std::collections::BTreeMap::new();
    let mut usage = None;
    let mut response_id = None;
    let mut model = None;
    let mut raw_finish = None;
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let Some(data) = line.trim().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(id) = value.get("id").and_then(Value::as_str) {
            response_id = Some(id.to_owned());
        }
        if let Some(name) = value.get("model").and_then(Value::as_str) {
            model = Some(name.to_owned());
        }
        if let Some(u) = value.get("usage") {
            usage = usage_from_value(u);
        }
        if let Some(choice) = value.pointer("/choices/0") {
            if let Some(content) = choice.pointer("/delta/content").and_then(Value::as_str) {
                text.push_str(content);
            }
            let delta_reasoning = choice
                .pointer("/delta/reasoning")
                .and_then(Value::as_str)
                .or_else(|| {
                    choice
                        .pointer("/delta/reasoning_content")
                        .and_then(Value::as_str)
                });
            if let Some(chunk) = delta_reasoning {
                reasoning.push_str(chunk);
            }
            if let Some(finish) = choice.get("finish_reason").and_then(Value::as_str) {
                raw_finish = Some(finish.to_owned());
            }
            if let Some(list) = choice
                .pointer("/delta/tool_calls")
                .and_then(Value::as_array)
            {
                for call in list {
                    let index = call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                    let entry = calls.entry(index).or_default();
                    if let Some(id) = call.get("id").and_then(Value::as_str) {
                        entry.0 = id.to_owned();
                    }
                    if let Some(name) = call.pointer("/function/name").and_then(Value::as_str) {
                        entry.1.push_str(name);
                    }
                    if let Some(args) = call.pointer("/function/arguments").and_then(Value::as_str)
                    {
                        entry.2.push_str(args);
                    }
                }
            }
        }
    }
    let ordered: Vec<(String, String, String)> = calls.into_values().collect();
    let thinking = (!reasoning.trim().is_empty())
        .then_some(ThinkingBlock {
            text: reasoning,
            signature: None,
        })
        .into_iter()
        .collect();
    let model = model.or_else(|| Some(requested_model.to_owned()));
    Ok(AssistantTurn {
        text,
        thinking,
        tool_calls: parse_tool_calls(&ordered),
        usage: finalize_usage(usage, model.as_deref().unwrap_or(requested_model)),
        api: Some("openai-completions".to_owned()),
        provider: Some("openrouter".to_owned()),
        model,
        stop_reason: finish_reason(raw_finish.as_deref()).map(str::to_owned),
        raw_stop_reason: raw_finish,
        response_id,
        duration_ms: started.elapsed().as_millis() as u64,
        thinking_level: None,
    })
}

impl Completer for OpenRouter {
    fn complete(&self, messages: &[Message], tools: &[ToolSpec]) -> anyhow::Result<AssistantTurn> {
        let started = std::time::Instant::now();
        let key = brainroom_ai::api_key()
            .ok_or_else(|| anyhow!("No OpenRouter API key found for the agent"))?;
        let tools_json: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect();
        let mut body = json!({
            "model": self.model,
            "messages": messages,
            "tools": tools_json,
            "tool_choice": "auto",
            "stream": true,
            "stream_options": { "include_usage": true },
            "max_tokens": 2048,
        });
        if let Some(level) = &self.thinking_level
            && level != "off"
        {
            body["reasoning"] = json!({ "effort": level });
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(self.timeout)
            .build()
            .context("build HTTP client")?;
        let mut last_error = None;
        for attempt in 0..3 {
            let response = client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Authorization", format!("Bearer {key}"))
                .header("Content-Type", "application/json")
                .header("HTTP-Referer", "https://github.com/emiliano-go/brainroom")
                .header("X-Title", "Brainroom")
                .json(&body)
                .send();
            match response {
                Ok(response) if response.status().is_success() => {
                    let stream = response
                        .headers()
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|value| value.to_str().ok())
                        .is_some_and(|value| value.contains("text/event-stream"));
                    if !stream {
                        let text = response.text().context("read OpenRouter response")?;
                        return parse_completion_json(&text, started, &self.model);
                    }
                    return parse_completion_stream(response, started, &self.model);
                }
                Ok(response) => {
                    let status = response.status();
                    let text = response.text().unwrap_or_default();
                    let retryable = status.as_u16() == 429 || status.is_server_error();
                    last_error = Some(anyhow!("OpenRouter {status}: {text}"));
                    if !retryable || attempt == 2 {
                        break;
                    }
                }
                Err(error) => {
                    last_error = Some(anyhow!(error).context("OpenRouter request failed"));
                    if attempt == 2 {
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500 * (1 << attempt)));
        }
        Err(last_error.unwrap_or_else(|| anyhow!("OpenRouter request failed")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex, mpsc};

    struct Fake {
        script: Mutex<VecDeque<AssistantTurn>>,
        seen: Arc<Mutex<Vec<Vec<Message>>>>,
    }

    impl Fake {
        fn new(script: Vec<AssistantTurn>) -> (Self, Arc<Mutex<Vec<Vec<Message>>>>) {
            let seen = Arc::new(Mutex::new(Vec::new()));
            (
                Fake {
                    script: Mutex::new(script.into()),
                    seen: seen.clone(),
                },
                seen,
            )
        }
    }

    impl Completer for Fake {
        fn complete(&self, messages: &[Message], _: &[ToolSpec]) -> anyhow::Result<AssistantTurn> {
            self.seen.lock().unwrap().push(messages.to_vec());
            self.script
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| anyhow!("fake script exhausted"))
        }
    }

    fn recv(rx: &Receiver<Event>) -> Event {
        rx.recv_timeout(Duration::from_secs(2))
            .expect("an agent event")
    }

    fn recv_app(rx: &Receiver<Event>) -> Event {
        loop {
            match recv(rx) {
                Event::Assistant(_) | Event::ToolResult { .. } | Event::Usage(_) => continue,
                other => return other,
            }
        }
    }

    #[test]
    fn nonstreaming_fallback_parses_tools_and_usage() {
        let body = r#"{
            "id":"gen-1","model":"m",
            "choices":[{"message":{"content":"hi","reasoning":"why","tool_calls":[{"id":"c1","function":{"name":"quiz","arguments":"{\"question\":\"Q\"}"}}]},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15,"cost":0.02}
        }"#;
        let turn = parse_completion_json(body, std::time::Instant::now(), "m").unwrap();
        assert_eq!(turn.text, "hi");
        assert_eq!(turn.thinking[0].text, "why");
        assert_eq!(turn.tool_calls[0].id, "c1");
        assert_eq!(turn.usage.unwrap().total_tokens, 15);
        assert_eq!(turn.stop_reason.as_deref(), Some("toolUse"));
    }

    #[test]
    fn usage_captures_cache_and_reasoning() {
        let usage = usage_from_value(&json!({
            "prompt_tokens": 100,
            "completion_tokens": 20,
            "total_tokens": 120,
            "cost": 0.01,
            "prompt_tokens_details": { "cached_tokens": 40 },
            "completion_tokens_details": { "reasoning_tokens": 5 }
        }))
        .unwrap();
        assert_eq!(usage.cache_read_tokens, 40);
        assert_eq!(usage.reasoning_tokens, 5);
        assert_eq!(usage.cost, Some(0.01));
    }

    #[test]
    fn assistant_record_matches_learnharness_shape() {
        let turn = AssistantTurn {
            text: "Here.".to_owned(),
            thinking: vec![ThinkingBlock {
                text: "Because.".to_owned(),
                signature: Some("[]".to_owned()),
            }],
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                name: "quiz".to_owned(),
                arguments: json!({ "question": "Q" }),
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cost: Some(0.25),
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                reasoning_tokens: 0,
            }),
            api: Some("openai-completions".to_owned()),
            provider: Some("openrouter".to_owned()),
            model: Some("m".to_owned()),
            stop_reason: Some("toolUse".to_owned()),
            raw_stop_reason: Some("tool_calls".to_owned()),
            response_id: Some("gen-1".to_owned()),
            duration_ms: 42,
            thinking_level: Some("medium".to_owned()),
        };
        let record = assistant_record(&turn);
        assert_eq!(record["content"][0]["type"], "thinking");
        assert_eq!(record["content"][1]["type"], "text");
        assert_eq!(record["content"][2]["type"], "toolCall");
        assert_eq!(record["usage"]["totalTokens"], 15);
        assert_eq!(record["usage"]["cost"]["total"], 0.25);
        assert_eq!(record["stopReason"], "toolUse");
        assert_eq!(record["rawStopReason"], "tool_calls");
        assert_eq!(record["durationMs"], 42);
    }

    #[test]
    fn quiz_waits_for_the_answer_then_continues() {
        let quiz = AssistantTurn {
            text: "Try this.".to_owned(),
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                name: "probe".to_owned(),
                arguments: json!({
                    "question": "What is 2+2?",
                    "options": [{ "label": "3" }, { "label": "4", "value": "four" }],
                    "correctAnswer": "four"
                }),
            }],
            usage: None,
            ..Default::default()
        };
        let done = AssistantTurn {
            text: "Correct.".to_owned(),
            tool_calls: vec![],
            usage: None,
            ..Default::default()
        };
        let (fake, seen) = Fake::new(vec![quiz, done]);
        let (in_tx, in_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || run(&fake, PathBuf::from("/tmp"), Vec::new(), in_rx, out_tx));

        in_tx.send(Input::User("teach me".to_owned())).unwrap();
        assert!(matches!(recv_app(&out_rx), Event::Text(t) if t == "Try this."));
        let Event::Quiz {
            call_id,
            question,
            options,
            correct,
            ..
        } = recv_app(&out_rx)
        else {
            panic!("expected a quiz");
        };
        assert_eq!(call_id, "call-1");
        assert_eq!(question, "What is 2+2?");
        assert_eq!(options, ["3", "4"]);
        assert_eq!(correct, [1]);

        in_tx
            .send(Input::ToolResult {
                call_id: "call-1".to_owned(),
                content: "4".to_owned(),
            })
            .unwrap();
        assert!(matches!(recv_app(&out_rx), Event::Text(t) if t == "Correct."));
        assert!(matches!(recv_app(&out_rx), Event::Done));

        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        let second = seen.last().unwrap();
        assert!(
            second
                .iter()
                .any(|m| m.get("role").and_then(Value::as_str) == Some("tool"))
        );
    }

    #[test]
    fn practice_poses_then_concludes_with_the_solution() {
        let problem = AssistantTurn {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "prob-1".to_owned(),
                name: "problem".to_owned(),
                arguments: json!({
                    "statement": "Differentiate x^2.",
                    "mode": "recall",
                    "modelSolution": "2x"
                }),
            }],
            usage: None,
            ..Default::default()
        };
        let conclusion = AssistantTurn {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "concl-1".to_owned(),
                name: "conclude".to_owned(),
                arguments: json!({ "outcome": "solved", "verdict": "Correct." }),
            }],
            usage: None,
            ..Default::default()
        };
        let done = AssistantTurn {
            text: "Next problem soon.".to_owned(),
            tool_calls: vec![],
            usage: None,
            ..Default::default()
        };
        let (fake, _) = Fake::new(vec![problem, conclusion, done]);
        let (in_tx, in_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || {
            run_with_mode(
                &fake,
                PathBuf::from("/tmp"),
                Vec::new(),
                in_rx,
                out_tx,
                Mode::Practice,
            )
        });

        in_tx
            .send(Input::User("practice derivatives".to_owned()))
            .unwrap();
        let Event::Problem {
            call_id,
            statement,
            mode,
            solution,
        } = recv_app(&out_rx)
        else {
            panic!("expected a problem");
        };
        assert_eq!(call_id, "prob-1");
        assert_eq!(statement, "Differentiate x^2.");
        assert_eq!(mode, "recall");
        assert_eq!(solution, "2x");
        assert!(matches!(recv_app(&out_rx), Event::Done));

        in_tx
            .send(Input::User("Your thinking:\n2x".to_owned()))
            .unwrap();
        let Event::Concluded {
            outcome,
            verdict,
            solution,
        } = recv_app(&out_rx)
        else {
            panic!("expected a conclusion");
        };
        assert_eq!(outcome, "solved");
        assert_eq!(verdict, "Correct.");
        assert_eq!(solution, "2x");
        assert!(matches!(recv_app(&out_rx), Event::Text(t) if t == "Next problem soon."));
        assert!(matches!(recv_app(&out_rx), Event::Done));
    }

    #[test]
    fn prober_tools_are_distinct() {
        let names: Vec<&str> = tools_for(Mode::Prober).iter().map(|t| t.name).collect();
        for required in ["probe", "recall", "ask_user_question", "probe_done"] {
            assert!(names.contains(&required), "missing {required}");
        }
        for teacher_only in ["quiz", "explain", "plan_init", "problem"] {
            assert!(!names.contains(&teacher_only), "prober has {teacher_only}");
        }
    }

    #[test]
    fn mermaid_nodes_parse_chains() {
        let ids = mermaid_node_ids("graph TD\n  a[Alpha] --> b[Beta]\n  c --> d[Delta]");
        assert_eq!(ids, ["a", "b", "c", "d"]);
    }

    #[test]
    fn plan_validation_finds_cycles_and_bad_slugs() {
        let cycle = parse_mermaid_plan("graph TD\n  a[A] --> b[B]\n  b --> a");
        assert!(cycle.problems.iter().any(|p| p.contains("cycle")));
        let bad = parse_mermaid_plan("graph TD\n  A[A] --> b[B]");
        assert!(bad.problems.iter().any(|p| p.contains("lowercase slug")));
        let good = parse_mermaid_plan("graph TD\n  a[A] --> b[B] --> c[C]");
        assert!(good.problems.is_empty());
        assert_eq!(good.nodes.len(), 3);
    }

    #[test]
    fn teacher_quiz_requires_a_known_plan_node() {
        let quiz = AssistantTurn {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                name: "quiz".to_owned(),
                arguments: json!({
                    "type": "check",
                    "nodeId": "missing",
                    "question": "Q",
                    "options": [{"label": "A"}, {"label": "B"}],
                    "correctAnswer": "A"
                }),
            }],
            usage: None,
            ..Default::default()
        };
        let done = AssistantTurn {
            text: "Retry.".to_owned(),
            tool_calls: vec![],
            usage: None,
            ..Default::default()
        };
        let (fake, _) = Fake::new(vec![quiz, done]);
        let (in_tx, in_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || run(&fake, PathBuf::from("/tmp"), Vec::new(), in_rx, out_tx));
        in_tx.send(Input::User("teach".to_owned())).unwrap();
        let mut error = false;
        let mut posed = false;
        loop {
            match recv(&out_rx) {
                Event::ToolResult {
                    is_error, content, ..
                } => {
                    assert!(is_error);
                    assert!(content.contains("not in this session's plan"));
                    error = true;
                }
                Event::Quiz { .. } => posed = true,
                Event::Done => break,
                _ => {}
            }
        }
        assert!(error);
        assert!(!posed);
    }

    #[test]
    fn practice_rejects_a_second_active_problem() {
        let initial = vec![serde_json::json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "old",
                "type": "function",
                "function": {"name": "problem", "arguments": "{\"statement\":\"old\"}"}
            }]
        })];
        let next = AssistantTurn {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "new".to_owned(),
                name: "problem".to_owned(),
                arguments: json!({"statement": "new", "mode": "recall", "modelSolution": "x"}),
            }],
            usage: None,
            ..Default::default()
        };
        let done = AssistantTurn {
            text: "Retry.".to_owned(),
            tool_calls: vec![],
            usage: None,
            ..Default::default()
        };
        let (fake, _) = Fake::new(vec![next, done]);
        let (in_tx, in_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || {
            run_with_mode(
                &fake,
                PathBuf::from("/tmp"),
                initial,
                in_rx,
                out_tx,
                Mode::Practice,
            )
        });
        in_tx.send(Input::User("next".to_owned())).unwrap();
        let mut error = false;
        let mut posed = false;
        loop {
            match recv(&out_rx) {
                Event::ToolResult {
                    is_error, content, ..
                } => {
                    assert!(is_error);
                    assert!(content.contains("already active"));
                    error = true;
                }
                Event::Problem { .. } => posed = true,
                Event::Done => break,
                _ => {}
            }
        }
        assert!(error);
        assert!(!posed);
    }

    #[test]
    fn graph_tools_are_gated_and_session_scoped() {
        let dir = std::env::temp_dir().join(format!("br-graph-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("graph")).unwrap();
        std::fs::write(
            dir.join("graph/nodes.jsonl"),
            "{\"id\":\"n1\",\"name\":\"Alpha\",\"provenance\":[{\"chatId\":\"chat_a\",\"messageId\":\"m1\"}]}\n{\"id\":\"n2\",\"name\":\"Beta\",\"provenance\":[{\"chatId\":\"chat_a\",\"messageId\":\"m2\"}]}\n{\"id\":\"n3\",\"name\":\"Gamma\",\"provenance\":[{\"chatId\":\"chat_b\",\"messageId\":\"m3\"}]}\n",
        )
        .unwrap();
        let sessions = list_sessions(&dir);
        assert!(sessions.contains("2 session(s)"));
        assert!(sessions.contains("id: chat_a"));
        let props = list_propositions(&dir, "chat_a");
        assert!(props.contains("2 proposition(s)"));
        assert!(props.contains("- Alpha"));
        assert!(!props.contains("statement"));
        let names: Vec<&str> = tools_for_session(Mode::Prober, false)
            .iter()
            .map(|tool| tool.name)
            .collect();
        assert!(!names.contains(&"list_sessions"));
        assert!(
            tools_for_session(Mode::Prober, true)
                .iter()
                .any(|tool| tool.name == "list_sessions")
        );
    }

    #[test]
    fn practice_mode_has_no_quiz_tool() {
        assert!(
            tools_for(Mode::Practice)
                .iter()
                .any(|t| t.name == "problem")
        );
        assert!(tools_for(Mode::Practice).iter().all(|t| t.name != "quiz"));
    }

    #[test]
    fn plan_and_sessions_tools_run_without_stopping() {
        let calls = AssistantTurn {
            text: String::new(),
            tool_calls: vec![
                ToolCall {
                    id: "plan".to_owned(),
                    name: "plan_init".to_owned(),
                    arguments: json!({ "mermaid": "graph TD\n  a[A] --> b[B]" }),
                },
                ToolCall {
                    id: "sessions".to_owned(),
                    name: "list_sessions".to_owned(),
                    arguments: json!({}),
                },
            ],
            usage: None,
            ..Default::default()
        };
        let done = AssistantTurn {
            text: "Planned.".to_owned(),
            tool_calls: vec![],
            usage: None,
            ..Default::default()
        };
        let (fake, seen) = Fake::new(vec![calls, done]);
        let (in_tx, in_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || run(&fake, PathBuf::from("/tmp"), Vec::new(), in_rx, out_tx));

        in_tx.send(Input::User("plan".to_owned())).unwrap();
        assert!(
            matches!(recv_app(&out_rx), Event::Plan { mermaid } if mermaid.contains("graph TD"))
        );
        assert!(matches!(recv_app(&out_rx), Event::Text(t) if t == "Planned."));
        assert!(matches!(recv_app(&out_rx), Event::Done));

        let seen = seen.lock().unwrap();
        let second = seen.last().unwrap();
        assert!(
            second
                .iter()
                .any(|m| m.get("tool_call_id").and_then(Value::as_str) == Some("sessions"))
        );
    }

    #[test]
    #[ignore = "needs OPENROUTER_API_KEY and network"]
    fn openrouter_practice_round_trip() {
        let model =
            brainroom_ai::model_id(&brainroom_ai::home_dir().join(".config/LearnHarness/data"));
        let agent = OpenRouter {
            model,
            timeout: Duration::from_secs(120),
            thinking_level: None,
        };
        let data = brainroom_ai::home_dir().join(".config/LearnHarness/data");
        let (in_tx, in_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || {
            run_with_mode(&agent, data, Vec::new(), in_rx, out_tx, Mode::Practice)
        });
        in_tx
            .send(Input::User(
                "Call the problem tool with statement \"What is 2+2?\", mode recall, and \
                 modelSolution \"4\". Then wait."
                    .to_owned(),
            ))
            .unwrap();
        let call_id = loop {
            match out_rx.recv_timeout(Duration::from_secs(120)) {
                Ok(Event::Problem { call_id, .. }) => break call_id,
                Ok(Event::Error(error)) => panic!("agent error: {error}"),
                Ok(_) => continue,
                Err(error) => panic!("no problem event: {error}"),
            }
        };
        in_tx
            .send(Input::ToolResult {
                call_id,
                content: "The learner answered: 4".to_owned(),
            })
            .unwrap();
        loop {
            match out_rx.recv_timeout(Duration::from_secs(120)) {
                Ok(Event::Concluded { solution, .. }) => {
                    assert!(solution.contains('4'));
                    break;
                }
                Ok(Event::Error(error)) => panic!("agent error after answer: {error}"),
                Ok(_) => {}
                Err(error) => panic!("no conclusion: {error}"),
            }
        }
    }

    #[test]
    #[ignore = "needs OPENROUTER_API_KEY and network"]
    fn openrouter_quiz_round_trip() {
        let model =
            brainroom_ai::model_id(&brainroom_ai::home_dir().join(".config/LearnHarness/data"));
        let agent = OpenRouter {
            model,
            timeout: Duration::from_secs(120),
            thinking_level: None,
        };
        let data = brainroom_ai::home_dir().join(".config/LearnHarness/data");
        let (in_tx, in_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || run(&agent, data, Vec::new(), in_rx, out_tx));
        in_tx
            .send(Input::User(
                "Call the quiz tool exactly once with question \"What is 2+2?\" and options \
                 \"3\" and \"4\". Do not write a question in prose."
                    .to_owned(),
            ))
            .unwrap();
        let call_id = loop {
            match out_rx.recv_timeout(Duration::from_secs(120)) {
                Ok(Event::Quiz { call_id, .. }) => break call_id,
                Ok(Event::Error(error)) => panic!("agent error: {error}"),
                Ok(_) => continue,
                Err(error) => panic!("no quiz event: {error}"),
            }
        };
        in_tx
            .send(Input::ToolResult {
                call_id,
                content: "The learner picked: 4".to_owned(),
            })
            .unwrap();
        let mut saw_text = false;
        loop {
            match out_rx.recv_timeout(Duration::from_secs(120)) {
                Ok(Event::Text(_)) => saw_text = true,
                Ok(Event::Done) => break,
                Ok(Event::Error(error)) => panic!("agent error after answer: {error}"),
                Ok(_) => {}
                Err(error) => panic!("no follow-up: {error}"),
            }
        }
        assert!(saw_text, "the agent said something after the answer");
    }

    #[test]
    #[ignore = "needs OPENROUTER_API_KEY and network"]
    fn openrouter_live_smoke() {
        let model =
            brainroom_ai::model_id(&brainroom_ai::home_dir().join(".config/LearnHarness/data"));
        let agent = OpenRouter {
            model,
            timeout: Duration::from_secs(90),
            thinking_level: None,
        };
        let turn = agent
            .complete(
                &[user_message("Reply with the single word: ready")],
                &tools(),
            )
            .expect("live completion");
        assert!(turn.text.to_lowercase().contains("ready") || !turn.tool_calls.is_empty());
    }
}
