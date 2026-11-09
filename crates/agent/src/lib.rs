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
            description: "List the learner's previous sessions.",
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "list_propositions",
            description: "List propositions in the learner's knowledge graph.",
            parameters: json!({
                "type": "object",
                "properties": { "session": { "type": "string" } }
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
            description: "List the learner's previous sessions.",
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ToolSpec {
            name: "list_propositions",
            description: "List propositions in the learner's knowledge graph.",
            parameters: json!({ "type": "object", "properties": { "session": { "type": "string" } } }),
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
                    "verdict": { "type": "string" }
                },
                "required": ["outcome", "verdict"]
            }),
        },
        ToolSpec {
            name: "list_propositions",
            description: "List propositions in the learner's knowledge graph.",
            parameters: json!({ "type": "object", "properties": { "session": { "type": "string" } } }),
        },
        ToolSpec {
            name: "list_sessions",
            description: "List the learner's previous sessions.",
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
                "cacheRead": 0,
                "cacheWrite": 0,
                "reasoning": 0,
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

fn list_sessions(data_dir: &Path) -> String {
    let store = brainroom_store::Store::new(data_dir.to_path_buf());
    serde_json::to_string(&store.list_chats()).unwrap_or_else(|_| "[]".to_owned())
}

fn list_propositions(data_dir: &Path) -> String {
    let path = data_dir.join("graph/nodes.jsonl");
    let Ok(text) = std::fs::read_to_string(path) else {
        return "[]".to_owned();
    };
    let nodes: Vec<Value> = text
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .map(|v| {
            json!({
                "name": v.get("name").and_then(Value::as_str).unwrap_or_default(),
                "statement": v.get("statement").and_then(Value::as_str).unwrap_or_default(),
            })
        })
        .collect();
    serde_json::to_string(&nodes).unwrap_or_else(|_| "[]".to_owned())
}

fn mermaid_node_ids(mermaid: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in mermaid.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("graph") || line.starts_with("flowchart") {
            continue;
        }
        for segment in line.split("-->") {
            let segment = segment.trim();
            if segment.is_empty() {
                continue;
            }
            let id = match segment.find('[') {
                Some(pos) => segment[..pos].trim(),
                None => segment.split_whitespace().next().unwrap_or(""),
            };
            if !id.is_empty()
                && id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'))
                && !ids.iter().any(|existing| existing == id)
            {
                ids.push(id.to_owned());
            }
        }
    }
    ids
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
            let turn = match completer.complete(&messages, &tools_for(mode)) {
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
            for call in &turn.tool_calls {
                match call.name.as_str() {
                    "problem" => {
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
                        let _ = emit.send(Event::Problem {
                            call_id: call.id.clone(),
                            statement,
                            mode: mode_name,
                            solution,
                        });
                        blocking = Some((call.id.clone(), String::new()));
                    }
                    "conclude" => {
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
                            outcome,
                            verdict,
                            solution: problem_solution.take().unwrap_or_default(),
                        });
                        messages.push(tool_result(&call.id, "Recorded."));
                        emit_tool_result(
                            &emit,
                            &call.id,
                            "conclude",
                            "Recorded.".to_owned(),
                            None,
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
                        plan_nodes.extend(mermaid_node_ids(&mermaid));
                        let _ = emit.send(Event::Plan { mermaid });
                        messages.push(tool_result(&call.id, "Plan recorded."));
                        emit_tool_result(
                            &emit,
                            &call.id,
                            "plan_init",
                            "Plan recorded.".to_owned(),
                            None,
                            false,
                            0,
                        );
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
                        let content = list_propositions(&data_dir);
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

/// OpenRouter chat completion with tool calling (non-streaming).
pub struct OpenRouter {
    pub model: String,
    pub timeout: Duration,
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
        let body = json!({
            "model": self.model,
            "messages": messages,
            "tools": tools_json,
            "tool_choice": "auto",
            "stream": false,
            "max_tokens": 2048,
        });
        let client = reqwest::blocking::Client::builder()
            .timeout(self.timeout)
            .build()
            .context("build HTTP client")?;
        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .header("HTTP-Referer", "https://github.com/emiliano-go/brainroom")
            .header("X-Title", "Brainroom")
            .json(&body)
            .send()
            .context("OpenRouter request failed")?;
        let status = response.status();
        let text = response.text().context("read OpenRouter response")?;
        if !status.is_success() {
            return Err(anyhow!("OpenRouter {status}: {text}"));
        }
        let value: Value = serde_json::from_str(&text).context("parse OpenRouter response")?;
        let message = value
            .pointer("/choices/0/message")
            .ok_or_else(|| anyhow!("OpenRouter response has no message: {text}"))?;
        let reply = message
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let mut tool_calls = Vec::new();
        if let Some(calls) = message.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                let id = call
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let name = call
                    .pointer("/function/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let raw = call
                    .pointer("/function/arguments")
                    .and_then(Value::as_str)
                    .unwrap_or("{}");
                let arguments =
                    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()));
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
        }
        let usage = value.get("usage").and_then(|u| {
            let prompt = u.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0);
            let completion = u
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let total = u
                .get("total_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(prompt + completion);
            (total > 0 || prompt > 0 || completion > 0).then_some(Usage {
                prompt_tokens: prompt,
                completion_tokens: completion,
                total_tokens: total,
                cost: u.get("cost").and_then(Value::as_f64),
            })
        });
        let model = value
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| Some(self.model.clone()));
        let response_id = value.get("id").and_then(Value::as_str).map(str::to_owned);
        let finish = value
            .pointer("/choices/0/finish_reason")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let stop_reason = finish.as_deref().map(|reason| match reason {
            "tool_calls" => "toolUse",
            "length" => "length",
            "content_filter" => "error",
            _ => "stop",
        });
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
        Ok(AssistantTurn {
            text: reply,
            thinking,
            tool_calls,
            usage,
            api: Some("openai-completions".to_owned()),
            provider: Some("openrouter".to_owned()),
            model,
            stop_reason: stop_reason.map(str::to_owned),
            raw_stop_reason: finish,
            response_id,
            duration_ms: started.elapsed().as_millis() as u64,
            thinking_level: None,
        })
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

        in_tx
            .send(Input::ToolResult {
                call_id: "prob-1".to_owned(),
                content: "2x".to_owned(),
            })
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
