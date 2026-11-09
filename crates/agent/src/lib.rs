//! The teaching agent: an OpenAI-style tool loop that stops for learner answers.

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
pub struct AssistantTurn {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
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
        question: String,
        options: Vec<String>,
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
    Teach,
    Practice,
}

pub fn tools() -> Vec<ToolSpec> {
    tools_for(Mode::Teach)
}

pub fn tools_for(mode: Mode) -> Vec<ToolSpec> {
    match mode {
        Mode::Teach => teach_tools(),
        Mode::Practice => practice_tools(),
    }
}

fn teach_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "quiz",
            description: "Ask the learner one multiple-choice question and wait for their answer.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "explanation": { "type": "string", "description": "Why the correct answer is correct." },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": { "label": { "type": "string" } },
                            "required": ["label"]
                        }
                    }
                },
                "required": ["question", "options"]
            }),
        },
        ToolSpec {
            name: "ask_user_question",
            description: "Ask the learner a focused setup question before teaching.",
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": { "label": { "type": "string" } },
                            "required": ["label"]
                        }
                    }
                },
                "required": ["question"]
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

fn system_message(mode: Mode) -> Message {
    let content = match mode {
        Mode::Teach => brainroom_ai::SYSTEM_PROMPT,
        Mode::Practice => {
            "You are Brainroom's practice coach. Pose exactly one problem with the problem tool, \
             wait for the learner's answer, then call conclude with a plain-language verdict and \
             the model solution. Never reveal the solution before conclude. Keep the learner working; \
             add a hint only when they are stuck."
        }
    };
    json!({ "role": "system", "content": content })
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

fn quiz_options(call: &ToolCall) -> Vec<String> {
    call.arguments
        .get("options")
        .and_then(Value::as_array)
        .map(|opts| {
            opts.iter()
                .filter_map(|o| {
                    o.get("label")
                        .and_then(Value::as_str)
                        .or_else(|| o.as_str())
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
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
    let mut messages = vec![system_message(mode)];
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
            messages.push(assistant_message(&turn));
            if turn.tool_calls.is_empty() {
                let _ = emit.send(Event::Done);
                break;
            }
            let mut blocking: Option<(String, String)> = None;
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
                    }
                    "quiz" | "ask_user_question" => {
                        let question = quiz_question(call);
                        let options = quiz_options(call);
                        let explanation = call
                            .arguments
                            .get("explanation")
                            .and_then(Value::as_str)
                            .map(str::to_owned);
                        let _ = emit.send(Event::Quiz {
                            call_id: call.id.clone(),
                            question: question.clone(),
                            options,
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
                        let _ = emit.send(Event::Plan { mermaid });
                        messages.push(tool_result(&call.id, "Plan recorded."));
                    }
                    "list_sessions" => {
                        messages.push(tool_result(&call.id, &list_sessions(&data_dir)));
                    }
                    "list_propositions" => {
                        messages.push(tool_result(&call.id, &list_propositions(&data_dir)));
                    }
                    "send_to_user" => {
                        let text = call
                            .arguments
                            .get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        let _ = emit.send(Event::Text(text));
                        messages.push(tool_result(&call.id, "Delivered."));
                    }
                    other => {
                        messages.push(tool_result(&call.id, &format!("Unknown tool: {other}")))
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
        Ok(AssistantTurn {
            text: reply,
            tool_calls,
            usage,
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

    #[test]
    fn quiz_waits_for_the_answer_then_continues() {
        let quiz = AssistantTurn {
            text: "Try this.".to_owned(),
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                name: "quiz".to_owned(),
                arguments: json!({
                    "question": "What is 2+2?",
                    "options": [{ "label": "3" }, { "label": "4" }]
                }),
            }],
            usage: None,
        };
        let done = AssistantTurn {
            text: "Correct.".to_owned(),
            tool_calls: vec![],
            usage: None,
        };
        let (fake, seen) = Fake::new(vec![quiz, done]);
        let (in_tx, in_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || run(&fake, PathBuf::from("/tmp"), Vec::new(), in_rx, out_tx));

        in_tx.send(Input::User("teach me".to_owned())).unwrap();
        assert!(matches!(recv(&out_rx), Event::Text(t) if t == "Try this."));
        let Event::Quiz {
            call_id,
            question,
            options,
            ..
        } = recv(&out_rx)
        else {
            panic!("expected a quiz");
        };
        assert_eq!(call_id, "call-1");
        assert_eq!(question, "What is 2+2?");
        assert_eq!(options, ["3", "4"]);

        in_tx
            .send(Input::ToolResult {
                call_id: "call-1".to_owned(),
                content: "4".to_owned(),
            })
            .unwrap();
        assert!(matches!(recv(&out_rx), Event::Text(t) if t == "Correct."));
        assert!(matches!(recv(&out_rx), Event::Done));

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
        };
        let conclusion = AssistantTurn {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "concl-1".to_owned(),
                name: "conclude".to_owned(),
                arguments: json!({ "outcome": "solved", "verdict": "Correct." }),
            }],
            usage: None,
        };
        let done = AssistantTurn {
            text: "Next problem soon.".to_owned(),
            tool_calls: vec![],
            usage: None,
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
        } = recv(&out_rx)
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
        } = recv(&out_rx)
        else {
            panic!("expected a conclusion");
        };
        assert_eq!(outcome, "solved");
        assert_eq!(verdict, "Correct.");
        assert_eq!(solution, "2x");
        assert!(matches!(recv(&out_rx), Event::Text(t) if t == "Next problem soon."));
        assert!(matches!(recv(&out_rx), Event::Done));
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
        };
        let done = AssistantTurn {
            text: "Planned.".to_owned(),
            tool_calls: vec![],
            usage: None,
        };
        let (fake, seen) = Fake::new(vec![calls, done]);
        let (in_tx, in_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        std::thread::spawn(move || run(&fake, PathBuf::from("/tmp"), Vec::new(), in_rx, out_tx));

        in_tx.send(Input::User("plan".to_owned())).unwrap();
        assert!(matches!(recv(&out_rx), Event::Plan { mermaid } if mermaid.contains("graph TD")));
        assert!(matches!(recv(&out_rx), Event::Text(t) if t == "Planned."));
        assert!(matches!(recv(&out_rx), Event::Done));

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
