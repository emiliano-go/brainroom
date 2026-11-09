//! The Brainroom window: sidebar, home graph, session and settings.

use crate::ai;
use crate::data::{self, Answered, Chat, Graph, Row, SessionRow};
use crate::md;
use crate::theme::{self, Theme, mono, prose, section_title, serif};
use rune::{
    Color, Cursor, Cx, El, LineCap, ScrollSpec, Shape, TextEdit, col, el, icon, row, text,
    text_input,
};

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Home,
    Session,
}

pub struct State {
    pub view: View,
    pub sidebar: bool,
    pub sessions: Vec<SessionRow>,
    pub graph: Graph,
    pub chat: Option<Chat>,
    pub settings_open: bool,
    pub settings_section: usize,
    pub filter: usize,
    pub mode: usize,
    pub search: TextEdit,
    pub graph_search: TextEdit,
    pub topic: TextEdit,
    pub thinking: TextEdit,
    pub practice_answer: TextEdit,
    pub quiz_selected: Option<usize>,
    pub confidence: f32,
    pub model: String,
    pub models: Vec<(String, String)>,
    pub model_menu_open: bool,
    pub effort_menu_open: bool,
    pub dictate_open: bool,
    pub effort: String,
    pub cost: String,
    pub tokens: u64,
    pub spend: f64,
    pub dark_override: Option<bool>,
    pub chains_open: bool,
    pub plan_open: bool,
    pub selected_node: Option<usize>,
    pub node_rename_open: bool,
    pub node_rename_text: TextEdit,
    pub active_id: Option<String>,
    pub renaming: Option<String>,
    pub rename_text: TextEdit,
    pub reply: TextEdit,
    agent: Option<ai::AgentHandle>,
    pending_call: Option<String>,
    pending_problem: bool,
    pub stream_text: String,
    pub stream_error: Option<String>,
    pub busy: bool,
    extracting: bool,
    extract_rx: Option<std::sync::mpsc::Receiver<Result<brainroom_extract::Stats, String>>>,
    pub extract_status: Option<String>,
}

impl State {
    pub fn new() -> State {
        let store = data::store();
        let sessions = data::sessions(&store);
        let graph = data::load_graph(&store);
        let model = store
            .strong_model()
            .map(|m| m.id)
            .unwrap_or_else(|| "nemotron-3-ultra-550b-a55b:free".to_owned());
        State {
            view: View::Home,
            sidebar: true,
            sessions,
            graph,
            chat: None,
            settings_open: false,
            settings_section: 0,
            filter: 0,
            mode: 0,
            search: TextEdit::new(""),
            graph_search: TextEdit::new(""),
            topic: TextEdit::new(""),
            thinking: TextEdit::new(""),
            practice_answer: TextEdit::new(""),
            quiz_selected: None,
            confidence: 0.5,
            model,
            models: data::model_slots(),
            model_menu_open: false,
            effort_menu_open: false,
            dictate_open: false,
            effort: data::effort(),
            cost: "$0.0000".to_owned(),
            tokens: 0,
            spend: 0.0,
            dark_override: data::appearance_dark(),
            chains_open: false,
            plan_open: false,
            selected_node: None,
            node_rename_open: false,
            node_rename_text: TextEdit::new(""),
            active_id: None,
            renaming: None,
            rename_text: TextEdit::new(""),
            reply: TextEdit::new(""),
            agent: None,
            pending_call: None,
            pending_problem: false,
            stream_text: String::new(),
            stream_error: None,
            busy: false,
            extracting: false,
            extract_rx: None,
            extract_status: None,
        }
    }

    pub fn open_first(&mut self) {
        if let Some(s) = self.sessions.first() {
            let id = s.id.clone();
            self.open_chat(&id);
        }
    }

    pub fn open_chat(&mut self, id: &str) {
        self.stop_stream();
        let store = data::store();
        self.chat = data::load_chat(&store, id);
        self.active_id = Some(id.to_owned());
        self.thinking = TextEdit::new("");
        self.reply = TextEdit::new("");
        self.quiz_selected = None;
        self.confidence = 0.5;
        self.view = View::Session;
    }

    pub fn import_notes(&mut self, path: std::path::PathBuf) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            self.stream_error = Some("Could not read the selected notes file".to_owned());
            return;
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Imported notes");
        let store = data::store();
        let record = store.create_chat(&format!("Imported: {name}"));
        self.active_id = Some(record.id);
        self.chat = None;
        self.send_message(&format!(
            "These are my notes. Teach me from them:\n\n{text}"
        ));
    }

    pub fn start_topic(&mut self) {
        let topic = self.topic.text.trim().to_owned();
        if topic.is_empty() {
            return;
        }
        self.topic = TextEdit::new("");
        self.active_id = None;
        self.send_message(&topic);
    }

    pub fn begin_rename(&mut self, id: &str) {
        if let Some(chat) = data::store().load_chat(id) {
            self.rename_text = TextEdit::new(chat.title);
            self.renaming = Some(id.to_owned());
        }
    }

    pub fn commit_rename(&mut self) {
        let Some(id) = self.renaming.take() else {
            return;
        };
        let title = self.rename_text.text.trim().to_owned();
        if title.is_empty() {
            return;
        }
        let store = data::store();
        store.rename_chat(&id, &title);
        self.sessions = data::sessions(&store);
        if self.active_id.as_deref() == Some(id.as_str()) {
            self.chat = data::load_chat(&store, &id);
        }
    }

    pub fn delete_session(&mut self, id: &str) {
        let store = data::store();
        if !store.delete_chat(id) {
            return;
        }
        self.sessions = data::sessions(&store);
        if self.active_id.as_deref() == Some(id) {
            self.stop_stream();
            self.active_id = None;
            self.chat = None;
            self.view = View::Home;
        }
    }

    fn history_messages(&self) -> Vec<serde_json::Value> {
        let Some(chat) = &self.chat else {
            return Vec::new();
        };
        let mut messages = Vec::new();
        for row in &chat.rows {
            match row {
                Row::User(text) => messages.push(serde_json::json!({
                    "role": "user", "content": text
                })),
                Row::Assistant(blocks) => messages.push(serde_json::json!({
                    "role": "assistant", "content": data::blocks_text(blocks)
                })),
                Row::Quiz(_) | Row::Problem(_) => {}
            }
        }
        if messages.len() > 16 {
            messages = messages.split_off(messages.len() - 16);
        }
        messages
    }

    fn persist_tool_call(&self, call_id: &str, name: &str, arguments: serde_json::Value) {
        let Some(id) = self.active_id.clone() else {
            return;
        };
        let store = data::store();
        if let Some(mut record) = store.load_chat(&id) {
            store.append_value(
                &mut record,
                serde_json::json!({
                    "role": "assistant",
                    "content": [{
                        "type": "toolCall",
                        "id": call_id,
                        "name": name,
                        "arguments": arguments,
                    }],
                    "timestamp": brainroom_core::now_millis(),
                }),
            );
        }
    }

    fn finish_turn(&mut self) {
        let text = self.stream_text.trim().to_owned();
        if !text.is_empty()
            && let Some(id) = self.active_id.clone()
        {
            let store = data::store();
            if let Some(mut record) = store.load_chat(&id) {
                store.append_message(&mut record, "assistant", &text);
            }
            self.chat = data::load_chat(&store, &id);
            self.sessions = data::sessions(&store);
        }
        if let Some(id) = self.active_id.clone()
            && self.chat.as_ref().is_none_or(|c| c.kind != "practice")
        {
            self.start_extraction(id);
        }
        self.stream_text.clear();
        self.busy = false;
    }

    fn start_extraction(&mut self, chat_id: String) {
        if self.extracting {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.extracting = true;
        self.extract_rx = Some(rx);
        std::thread::spawn(move || {
            let data = crate::data::data_dir();
            let model = brainroom_ai::model_id(&data);
            let extractor = brainroom_extract::OpenRouterExtractor {
                model,
                timeout: std::time::Duration::from_secs(120),
            };
            let result =
                brainroom_extract::run(&extractor, &data, &chat_id).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
    }

    pub fn send_message(&mut self, text: &str) {
        let text = text.trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.stop_stream();
        let store = data::store();
        let (id, mut record) = match &self.active_id {
            Some(id) => match store.load_chat(id) {
                Some(record) => (id.clone(), record),
                None => {
                    let mut record = store.create_chat(&title_from(&text));
                    if self.mode == 1 {
                        record.kind = Some("practice".to_owned());
                        store.save_chat(&record);
                    }
                    (record.id.clone(), record)
                }
            },
            None => {
                let mut record = store.create_chat(&title_from(&text));
                if self.mode == 1 {
                    record.kind = Some("practice".to_owned());
                    store.save_chat(&record);
                }
                (record.id.clone(), record)
            }
        };
        store.append_message(&mut record, "user", &text);
        self.active_id = Some(id.clone());
        self.sessions = data::sessions(&store);
        self.chat = data::load_chat(&store, &id);
        self.view = View::Session;
        self.reply = TextEdit::new("");
        self.stream_text.clear();
        self.stream_error = None;
        self.pending_call = None;
        self.busy = true;
        let mut history = self.history_messages();
        if history
            .last()
            .and_then(|m| m.get("role"))
            .and_then(|r| r.as_str())
            == Some("user")
        {
            history.pop();
        }
        let practice = self.chat.as_ref().is_some_and(|c| c.kind == "practice");
        let agent = ai::start(history, practice);
        let _ = agent.inbox.send(brainroom_agent::Input::User(text));
        self.agent = Some(agent);
    }

    pub fn stop_stream(&mut self) {
        self.agent = None;
        self.pending_call = None;
        self.pending_problem = false;
        self.busy = false;
    }

    /// Drain the background stream into the view state; true when it changed.
    /// Drain the agent's events into the view state; true when it changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        let extraction = self.extract_rx.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(Err("extraction thread stopped".to_owned()))
            }
        });
        if let Some(result) = extraction {
            self.extract_rx = None;
            self.extracting = false;
            match result {
                Ok(stats) => {
                    self.extract_status = Some(format!(
                        "Extracted {} ideas · {} chains",
                        stats.nodes, stats.chains
                    ));
                    self.graph = data::load_graph(&data::store());
                }
                Err(error) => self.extract_status = Some(format!("Extraction failed: {error}")),
            }
            changed = true;
        }
        let mut batch = Vec::new();
        if let Some(agent) = &self.agent {
            while let Ok(event) = agent.events.try_recv() {
                batch.push(event);
            }
        }
        if batch.is_empty() {
            return changed;
        }
        for event in batch {
            match event {
                brainroom_agent::Event::Text(text) => {
                    if !self.stream_text.is_empty() {
                        self.stream_text.push('\n');
                    }
                    self.stream_text.push_str(&text);
                }
                brainroom_agent::Event::Quiz {
                    call_id,
                    question,
                    options,
                    ..
                } => {
                    if let Some(chat) = &mut self.chat {
                        chat.rows.push(Row::Quiz(data::Quiz {
                            question: question.clone(),
                            options: options.clone(),
                            answered: None,
                        }));
                    }
                    let arg_options: Vec<serde_json::Value> = options
                        .iter()
                        .map(|label| serde_json::json!({ "label": label }))
                        .collect();
                    self.persist_tool_call(
                        &call_id,
                        "quiz",
                        serde_json::json!({ "question": question, "options": arg_options }),
                    );
                    self.pending_call = Some(call_id);
                    self.pending_problem = false;
                    self.busy = false;
                }
                brainroom_agent::Event::Problem {
                    call_id,
                    statement,
                    mode,
                    solution,
                } => {
                    if let Some(chat) = &mut self.chat {
                        chat.rows.push(Row::Problem(data::Problem {
                            statement: statement.clone(),
                            mode: mode.clone(),
                            solution: solution.clone(),
                            concluded: None,
                        }));
                    }
                    self.persist_tool_call(
                        &call_id,
                        "problem",
                        serde_json::json!({
                            "statement": statement,
                            "mode": mode,
                            "modelSolution": solution,
                        }),
                    );
                    self.pending_call = Some(call_id);
                    self.pending_problem = true;
                    self.busy = false;
                }
                brainroom_agent::Event::Concluded {
                    outcome, verdict, ..
                } => {
                    if let Some(chat) = &mut self.chat
                        && let Some(Row::Problem(problem)) = chat
                            .rows
                            .iter_mut()
                            .rev()
                            .find(|r| matches!(r, Row::Problem(_)))
                    {
                        problem.concluded = Some(data::Concluded {
                            outcome: outcome.clone(),
                            verdict: verdict.clone(),
                        });
                    }
                    self.persist_tool_call(
                        "",
                        "conclude",
                        serde_json::json!({ "outcome": outcome, "verdict": verdict }),
                    );
                    self.pending_problem = false;
                }
                brainroom_agent::Event::Plan { mermaid } => {
                    self.persist_tool_call(
                        "",
                        "plan_init",
                        serde_json::json!({ "mermaid": mermaid }),
                    );
                    if let Some(chat) = &mut self.chat {
                        chat.plan = Some(data::parse_plan(&mermaid));
                    }
                }
                brainroom_agent::Event::Usage(usage) => {
                    self.tokens = self.tokens.saturating_add(usage.total_tokens);
                    if let Some(cost) = usage.cost {
                        self.spend += cost;
                        self.cost = format!("${:.4}", self.spend);
                    }
                }
                brainroom_agent::Event::Done => {
                    self.finish_turn();
                }
                brainroom_agent::Event::Error(error) => {
                    self.stream_error = Some(error);
                    self.busy = false;
                }
            }
        }
        true
    }

    pub fn delete_selected_node(&mut self) {
        let Some(index) = self.selected_node.take() else {
            return;
        };
        if index >= self.graph.nodes.len() {
            return;
        }
        self.graph.nodes.remove(index);
        self.graph.edges = self
            .graph
            .edges
            .iter()
            .filter_map(|(a, b)| {
                if *a == index || *b == index {
                    return None;
                }
                Some((
                    if *a > index { *a - 1 } else { *a },
                    if *b > index { *b - 1 } else { *b },
                ))
            })
            .collect();
    }

    pub fn demo_quiz(&mut self) {
        self.open_first();
        if let Some(chat) = &mut self.chat {
            chat.id = "demo_missing_chat".to_owned();
            chat.rows.push(Row::Quiz(data::Quiz {
                question: "What is the derivative of $x^2$?".to_owned(),
                options: vec!["$x$".to_owned(), "$2x$".to_owned(), "$x^2$".to_owned()],
                answered: None,
            }));
        }
        self.pending_call = Some("demo-quiz".to_owned());
        self.pending_problem = false;
        self.busy = false;
    }

    pub fn demo_problem(&mut self) {
        self.open_first();
        if let Some(chat) = &mut self.chat {
            chat.id = "demo_missing_chat".to_owned();
            chat.kind = "practice".to_owned();
            chat.rows.push(Row::Problem(data::Problem {
                statement: "Differentiate $f(x) = x^2 + 3x$.".to_owned(),
                mode: "recall".to_owned(),
                solution: "$f'(x) = 2x + 3$".to_owned(),
                concluded: None,
            }));
        }
        self.pending_call = Some("demo-problem".to_owned());
        self.pending_problem = true;
        self.busy = false;
    }

    pub fn submit_problem(&mut self) {
        let Some(call_id) = self.pending_call.take() else {
            return;
        };
        let answer = self.practice_answer.text.trim().to_owned();
        if answer.is_empty() {
            self.pending_call = Some(call_id);
            return;
        }
        self.practice_answer = TextEdit::new("");
        self.pending_problem = false;
        if let Some(id) = self.active_id.clone()
            && let Some(mut record) = data::store().load_chat(&id)
        {
            data::store().append_value(
                &mut record,
                serde_json::json!({
                    "role": "toolResult",
                    "toolName": "problem",
                    "content": answer,
                    "timestamp": brainroom_core::now_millis(),
                }),
            );
        }
        if let Some(agent) = &self.agent {
            let _ = agent.inbox.send(brainroom_agent::Input::ToolResult {
                call_id,
                content: format!("The learner's answer:\n{answer}"),
            });
            self.busy = true;
        }
    }

    pub fn submit_quiz(&mut self) {
        if self.pending_problem {
            return;
        }
        let Some(chat) = &mut self.chat else { return };
        let Some(Row::Quiz(quiz)) = chat
            .rows
            .iter_mut()
            .rev()
            .find(|r| matches!(r, Row::Quiz(_)))
        else {
            return;
        };
        if quiz.answered.is_some() {
            return;
        }
        let chosen = self.quiz_selected;
        let correct = chosen.is_some_and(|i| i < quiz.options.len());
        quiz.answered = Some(Answered {
            chosen,
            correct: Some(correct),
        });
        let answer = match chosen {
            Some(i) if i < quiz.options.len() => quiz.options[i].clone(),
            _ => "I don't know".to_owned(),
        };
        let confidence = (self.confidence * 100.0).round() as i32;
        let thinking = self.thinking.text.trim().to_owned();
        let id = chat.id.clone();
        self.thinking = TextEdit::new("");
        let store = data::store();
        if let Some(mut record) = store.load_chat(&id) {
            let result = serde_json::json!({
                "kind": "quiz",
                "answers": [{ "index": chosen.filter(|i| *i < 100), "label": answer }],
                "correct": correct,
                "confidence": confidence,
                "thinking": thinking,
            });
            store.append_value(
                &mut record,
                serde_json::json!({
                    "role": "toolResult",
                    "toolName": "quiz",
                    "content": result.to_string(),
                    "timestamp": brainroom_core::now_millis(),
                }),
            );
        }
        if let Some(call_id) = self.pending_call.take() {
            let mut content = format!("The learner answered: {answer}. Confidence: {confidence}%.");
            if !thinking.is_empty() {
                content.push_str(&format!(" Their reasoning: {thinking}"));
            }
            if let Some(agent) = &self.agent {
                let _ = agent
                    .inbox
                    .send(brainroom_agent::Input::ToolResult { call_id, content });
                self.busy = true;
            }
        } else {
            let mut message = format!("My answer: {answer}. Confidence: {confidence}%.");
            if !thinking.is_empty() {
                message.push_str(&format!(" My reasoning: {thinking}"));
            }
            self.send_message(&message);
        }
    }
}

fn dot(_th: &Theme, c: Color) -> El {
    el().size(12.0, 12.0).radius(6.0).fill(c)
}

fn icon_button(key: &str, glyph: &'static rune::Icon, size: f32, th: &Theme, color: Color) -> El {
    el().key(key)
        .size(size + 14.0, size + 14.0)
        .center()
        .radius(8.0)
        .child(icon(glyph, size).color(color))
        .hover(th.hover)
        .press(th.pressed)
        .cursor(Cursor::Pointer)
}

fn title_from(prompt: &str) -> String {
    let trimmed = prompt.trim();
    let text: String = trimmed.chars().take(64).collect();
    if trimmed.chars().count() > 64 {
        format!("{text}…")
    } else if text.is_empty() {
        "Untitled session".to_owned()
    } else {
        text
    }
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let th = theme::theme(s.dark_override.unwrap_or_else(|| cx.system_dark()));
    if s.settings_open {
        return el()
            .key("root")
            .w_full()
            .h_full()
            .fill(th.bg)
            .child(settings(s, &th));
    }
    if s.chains_open {
        return el()
            .key("root")
            .w_full()
            .h_full()
            .fill(th.bg)
            .child(chains(s, &th));
    }
    if s.plan_open {
        return el()
            .key("root")
            .w_full()
            .h_full()
            .fill(th.bg)
            .child(plan_window(s, &th));
    }
    if s.model_menu_open {
        return el()
            .key("root")
            .w_full()
            .h_full()
            .fill(th.bg)
            .child(model_menu(s, &th));
    }
    if s.effort_menu_open {
        return el()
            .key("root")
            .w_full()
            .h_full()
            .fill(th.bg)
            .child(effort_menu(s, &th));
    }
    if s.dictate_open {
        return el()
            .key("root")
            .w_full()
            .h_full()
            .fill(th.bg)
            .child(dictate_modal(s, &th));
    }
    if s.node_rename_open {
        return el()
            .key("root")
            .w_full()
            .h_full()
            .fill(th.bg)
            .child(node_rename_modal(s, &th));
    }
    el().key("root")
        .w_full()
        .h_full()
        .fill(th.bg)
        .child(shell(s, &th))
}

fn shell(s: &State, th: &Theme) -> El {
    let mut root = row().w_full().h_full().fill(th.bg);
    if s.sidebar {
        root = root.child(sidebar(s, th));
    }
    let content = match s.view {
        View::Home => home(s, th),
        View::Session => session(s, th),
    };
    root.child(content.grow(1.0))
}

fn sidebar(s: &State, th: &Theme) -> El {
    let mut list = col().w_full().gap(2.0);
    let query = s.search.text.to_lowercase();
    for (i, session) in s.sessions.iter().enumerate() {
        if !query.is_empty() && !session.title.to_lowercase().contains(&query) {
            continue;
        }
        let practice = session.kind == "practice";
        if (s.filter == 0 && practice) || (s.filter == 1 && !practice) {
            continue;
        }
        let id = session.id.clone();
        let active = s.chat.as_ref().is_some_and(|c| c.id == id);
        let selected = active && s.view == View::Session;
        let mut item = col()
            .key(format!("session-{i}"))
            .w_full()
            .gap(3.0)
            .px(12.0)
            .py(9.0)
            .radius(8.0)
            .fill(if selected {
                th.card
            } else {
                Color::TRANSPARENT
            })
            .border(
                if selected { 1.0 } else { 0.0 },
                if selected {
                    th.border_soft
                } else {
                    Color::TRANSPARENT
                },
            );
        if s.renaming.as_deref() == Some(id.as_str()) {
            item = item.child(
                text_input(&s.rename_text, mono(14.0, 500.0))
                    .key(format!("rename-{i}"))
                    .single_line()
                    .w_full()
                    .h(26.0)
                    .pt(3.0)
                    .color(th.fg)
                    .fill(th.card)
                    .border(1.0, th.border)
                    .radius(4.0)
                    .on_edit(|s: &mut State| &mut s.rename_text)
                    .on_submit(|s: &mut State, _| s.commit_rename()),
            );
        } else {
            let mut title_row = row().w_full().items_center().child(
                text(session.title.clone())
                    .font(
                        rune::TextStyle::new(15.0, 600.0)
                            .family(rune::Family::Named("Source Sans 3"))
                            .lh(21.0),
                    )
                    .color(th.fg)
                    .grow(1.0),
            );
            if selected {
                let delete_id = id.clone();
                title_row = title_row.child(
                    text("✕".to_owned())
                        .key(format!("delete-{i}"))
                        .font(mono(11.0, 400.0))
                        .color(th.muted)
                        .cursor(Cursor::Pointer)
                        .on_press(move |s: &mut State| s.delete_session(&delete_id)),
                );
            }
            let rename_id = id.clone();
            item = item
                .child(title_row)
                .on_double_press(move |s: &mut State| s.begin_rename(&rename_id));
        }
        let open_id = id.clone();
        item = item
            .child(
                text(session.status.clone())
                    .font(mono(11.0, 400.0))
                    .color(th.muted),
            )
            .hover(th.hover)
            .press(th.pressed)
            .cursor(Cursor::Pointer)
            .on_press(move |s: &mut State| s.open_chat(&open_id));
        list = list.child(item);
    }

    let filter = |index: usize, label: &str, s: &State, th: &Theme| {
        let selected = s.filter == index;
        el().key(format!("filter-{index}"))
            .grow(1.0)
            .h(28.0)
            .center()
            .radius(6.0)
            .fill(if selected {
                th.raised
            } else {
                Color::TRANSPARENT
            })
            .child(
                text(label.to_owned())
                    .font(mono(11.0, 500.0))
                    .color(if selected { th.fg } else { th.muted }),
            )
            .hover(th.hover)
            .press(th.pressed)
            .cursor(Cursor::Pointer)
            .on_press(move |s: &mut State| s.filter = index)
    };

    let top = row()
        .w_full()
        .h(58.0)
        .px(18.0)
        .items_center()
        .gap(9.0)
        .child(
            icon(&theme::ICON_MARK, 22.0)
                .color(th.fg)
                .cursor(Cursor::Pointer)
                .on_press(|s: &mut State| s.view = View::Home),
        )
        .child(el().grow(1.0))
        .child(
            icon_button("toggle-sidebar", &theme::PANEL_LEFT, 18.0, th, th.muted)
                .on_press(|s: &mut State| s.sidebar = !s.sidebar),
        )
        .window_drag();

    let search = text_input(&s.search, mono(13.0, 400.0))
        .key("session-search")
        .single_line()
        .h(32.0)
        .px(10.0)
        .pt(7.0)
        .placeholder("Search…", th.faint)
        .color(th.fg)
        .fill(th.card)
        .border(1.0, th.border_soft)
        .radius(8.0)
        .on_edit(|s: &mut State| &mut s.search);

    let body = col()
        .w_full()
        .gap(14.0)
        .px(14.0)
        .pt(6.0)
        .grow(1.0)
        .shrink()
        .min_h(0.0)
        .child(
            col()
                .w_full()
                .gap(6.0)
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .items_center()
                        .child(icon(&theme::PLUS, 15.0).color(th.fg))
                        .child(text("TEACHING").font(mono(11.0, 500.0)).color(th.fg))
                        .hover(th.hover)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| {
                            s.view = View::Home;
                            s.topic = TextEdit::new("");
                        }),
                )
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .items_center()
                        .child(icon(&theme::DUMBBELL, 15.0).color(th.muted))
                        .child(text("PRACTICE").font(mono(11.0, 500.0)).color(th.muted))
                        .hover(th.hover)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| {
                            s.mode = 1;
                            s.view = View::Home;
                        }),
                ),
        )
        .child(
            col()
                .w_full()
                .gap(8.0)
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .items_center()
                        .child(text("SESSIONS").font(mono(11.0, 500.0)).color(th.muted))
                        .child(search.grow(1.0)),
                )
                .child(
                    row()
                        .w_full()
                        .gap(2.0)
                        .padding(2.0)
                        .radius(8.0)
                        .fill(th.card)
                        .child(filter(0, "TEACHING", s, th))
                        .child(filter(1, "PRACTICE", s, th))
                        .child(filter(2, "ALL", s, th)),
                ),
        )
        .child(
            col()
                .key("session-list")
                .w_full()
                .gap(2.0)
                .grow(1.0)
                .shrink()
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(list),
        );

    let footer = col()
        .w_full()
        .gap(10.0)
        .px(14.0)
        .pb(14.0)
        .child(
            col()
                .w_full()
                .gap(8.0)
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .items_center()
                        .child(
                            text("BACKGROUND AGENTS")
                                .font(mono(11.0, 500.0))
                                .color(th.muted),
                        )
                        .child(el().grow(1.0)),
                )
                .child(
                    row()
                        .w_full()
                        .gap(8.0)
                        .items_center()
                        .child(icon(&theme::THINKING, 14.0).color(th.muted))
                        .child(
                            text("No active jobs".to_owned())
                                .font(mono(11.0, 400.0))
                                .color(th.faint),
                        ),
                ),
        )
        .child(
            row()
                .key("settings-button")
                .w_full()
                .gap(9.0)
                .items_center()
                .px(8.0)
                .py(7.0)
                .radius(8.0)
                .child(icon(&theme::SETTINGS, 16.0).color(th.muted))
                .child(text("SETTINGS").font(mono(11.0, 500.0)).color(th.muted))
                .hover(th.hover)
                .press(th.pressed)
                .cursor(Cursor::Pointer)
                .on_press(|s: &mut State| s.settings_open = true),
        );

    col()
        .key("sidebar")
        .w(288.0)
        .h_full()
        .fill(th.bg)
        .border_sides([0.0, 1.0, 0.0, 0.0], th.border_soft)
        .child(top)
        .child(body)
        .child(footer)
}

fn top_pill(key: &str, glyph: &'static rune::Icon, label: &str, th: &Theme, filled: bool) -> El {
    row()
        .key(key)
        .h(30.0)
        .px(12.0)
        .gap(7.0)
        .items_center()
        .radius(8.0)
        .fill(if filled { th.card } else { Color::TRANSPARENT })
        .border(1.0, if filled { th.border_soft } else { th.border })
        .child(icon(glyph, 14.0).color(if filled { th.fg } else { th.muted }))
        .child(
            text(label.to_owned())
                .font(mono(11.0, 500.0))
                .color(if filled { th.fg } else { th.muted }),
        )
        .hover(th.hover)
        .cursor(Cursor::Pointer)
}

fn graph_box(s: &State, th: &Theme, w: f32, h: f32) -> El {
    let g = &s.graph;
    let mut edges = Shape::new();
    let mut dots = Shape::new();
    let px = |n: &data::Node| [80.0 + n.x * (w - 160.0), 70.0 + n.y * (h - 140.0)];
    for (a, b) in &g.edges {
        if let (Some(a), Some(b)) = (g.nodes.get(*a), g.nodes.get(*b)) {
            edges = edges.line(
                px(a),
                px(b),
                0.6,
                LineCap::Butt,
                Color {
                    a: 0.35,
                    ..th.muted
                },
            );
        }
    }
    for node in &g.nodes {
        let c = px(node);
        let mut pts = Vec::with_capacity(10);
        for i in 0..10 {
            let a = i as f32 * std::f32::consts::TAU / 10.0;
            pts.push([c[0] + a.cos() * 2.6, c[1] + a.sin() * 2.6]);
        }
        dots = dots.fill(pts, Color { a: 0.85, ..th.fg });
    }
    let mut layer = col().size(w, h).absolute(0.0, 0.0);
    for (i, (_title, x, y)) in g.clusters.iter().enumerate() {
        let cx = 80.0 + x * (w - 160.0);
        let cy = 70.0 + y * (h - 140.0);
        let glow = match i % 4 {
            0 => Color {
                r: 0.45,
                g: 0.55,
                b: 0.9,
                a: 0.10,
            },
            1 => Color {
                r: 0.9,
                g: 0.55,
                b: 0.45,
                a: 0.09,
            },
            2 => Color {
                r: 0.4,
                g: 0.8,
                b: 0.55,
                a: 0.09,
            },
            _ => Color {
                r: 0.7,
                g: 0.45,
                b: 0.9,
                a: 0.09,
            },
        };
        let clear = Color { a: 0.0, ..glow };
        layer = layer.child(
            el().size(520.0, 420.0)
                .absolute(cx - 260.0, cy - 210.0)
                .radius(260.0)
                .radial_gradient([0.5, 0.5], 260.0, [(0.0, glow), (1.0, clear)])
                .a11y_hidden(),
        );
    }
    layer = layer.child(
        el().size(w, h)
            .absolute(0.0, 0.0)
            .shape(edges)
            .a11y_hidden(),
    );
    layer = layer.child(el().size(w, h).absolute(0.0, 0.0).shape(dots).a11y_hidden());
    for (i, node) in g.nodes.iter().enumerate() {
        let c = px(node);
        layer = layer.child(
            text(node.name.clone())
                .key(format!("node-{i}"))
                .font(mono(7.0, 400.0))
                .color(Color {
                    a: 0.55,
                    ..th.muted
                })
                .max_w(96.0)
                .absolute(c[0] + 4.0, c[1] - 4.0),
        );
    }
    for (i, node) in g.nodes.iter().enumerate() {
        let c = px(node);
        layer = layer.child(
            el().key(format!("node-hit-{i}"))
                .size(14.0, 14.0)
                .absolute(c[0] - 7.0, c[1] - 7.0)
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| s.selected_node = Some(i)),
        );
    }
    for (i, (title, x, y)) in g.clusters.iter().enumerate() {
        let cx = 80.0 + x * (w - 160.0);
        let cy = 70.0 + y * (h - 140.0);
        layer = layer.child(
            text(title.clone())
                .key(format!("cluster-{i}"))
                .font(serif(14.0, 400.0))
                .color(Color {
                    a: 0.55,
                    ..th.muted
                })
                .absolute(cx - 80.0, cy - 8.0),
        );
    }
    layer
}

fn home(s: &State, th: &Theme) -> El {
    let search = text_input(&s.graph_search, mono(13.0, 400.0))
        .key("graph-search")
        .single_line()
        .w(620.0)
        .h(34.0)
        .px(34.0)
        .pt(8.0)
        .placeholder("Search nodes…", th.faint)
        .color(th.fg)
        .fill(th.card)
        .border(1.0, th.border_soft)
        .radius(10.0)
        .on_edit(|s: &mut State| &mut s.graph_search);

    let top = row()
        .w_full()
        .h(64.0)
        .px(24.0)
        .items_center()
        .gap(10.0)
        .child(if s.sidebar {
            el()
        } else {
            icon_button(
                "home-toggle-sidebar",
                &theme::PANEL_LEFT,
                17.0,
                th,
                th.muted,
            )
            .on_press(|s: &mut State| s.sidebar = true)
        })
        .child(el().grow(1.0))
        .child(
            row().gap(0.0).items_center().child(search).child(
                el().size(14.0, 14.0)
                    .absolute(12.0, 10.0)
                    .child(icon(&theme::SEARCH, 14.0).color(th.faint))
                    .a11y_hidden(),
            ),
        )
        .child(
            top_pill("chains", &theme::CHAIN, "Chains", th, true)
                .on_press(|s: &mut State| s.chains_open = true),
        )
        .child(
            top_pill("import-notes", &theme::UPLOAD, "Import notes…", th, true).on_press_tx(
                |_: &mut State, tx: &mut rune::Tx| {
                    let options = rune::FileDialogOptions {
                        title: "Import notes".to_owned(),
                        ..Default::default()
                    }
                    .filter("Notes", &["md", "txt", "markdown"]);
                    tx.open_file(options, |s: &mut State, result, _| {
                        if let Ok(Some(path)) = result {
                            s.import_notes(path);
                        }
                    });
                },
            ),
        )
        .child(
            col()
                .gap(0.0)
                .items_end()
                .child(
                    icon_button("home-settings", &theme::SETTINGS, 17.0, th, th.muted)
                        .on_press(|s: &mut State| s.settings_open = true),
                )
                .child(text(s.cost.clone()).font(mono(10.0, 400.0)).color(th.muted)),
        )
        .window_drag();

    let composer = col()
        .w(720.0)
        .gap(8.0)
        .child(
            row()
                .w_full()
                .h(56.0)
                .px(18.0)
                .items_center()
                .gap(10.0)
                .radius(14.0)
                .fill(th.card)
                .border(1.0, th.border)
                .child(
                    text_input(&s.topic, prose())
                        .key("topic")
                        .grow(1.0)
                        .shrink()
                        .h(40.0)
                        .pt(8.0)
                        .placeholder("What do you want to learn?", th.faint)
                        .color(th.fg)
                        .fill(Color::TRANSPARENT)
                        .on_edit(|s: &mut State| &mut s.topic)
                        .on_submit(|s: &mut State, _| s.start_topic()),
                )
                .child(
                    el().size(34.0, 34.0)
                        .center()
                        .radius(9.0)
                        .fill(th.raised)
                        .child(icon(&theme::ARROW_UP, 17.0).color(
                            if s.topic.text.trim().is_empty() {
                                th.faint
                            } else {
                                th.fg
                            },
                        ))
                        .hover(th.hover)
                        .cursor(Cursor::Pointer),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(8.0)
                .items_center()
                .child(
                    row()
                        .h(30.0)
                        .padding(2.0)
                        .gap(2.0)
                        .radius(8.0)
                        .fill(th.card)
                        .border(1.0, th.border_soft)
                        .child(mode_tab(0, "TEACH", &theme::PLUS, s, th))
                        .child(mode_tab(1, "PRACTICE", &theme::DUMBBELL, s, th)),
                )
                .child(
                    row()
                        .key("dictate")
                        .h(30.0)
                        .grow(1.0)
                        .items_center()
                        .justify_center()
                        .gap(7.0)
                        .radius(8.0)
                        .fill(th.card)
                        .border(1.0, th.border_soft)
                        .child(icon(&theme::MIC, 14.0).color(th.muted))
                        .child(
                            text("Dictate".to_owned())
                                .font(mono(11.0, 400.0))
                                .color(th.muted),
                        )
                        .hover(th.hover)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| s.dictate_open = true),
                )
                .child(model_pill(s, th))
                .child(
                    top_pill("effort", &theme::CHEVRON_DOWN, &s.effort, th, true)
                        .on_press(|s: &mut State| s.effort_menu_open = true),
                ),
        );

    col()
        .w_full()
        .h_full()
        .child(top)
        .child(
            col()
                .w_full()
                .grow(1.0)
                .shrink()
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(
                    col()
                        .w_full()
                        .h_full()
                        .center()
                        .child(graph_box(s, th, 1040.0, 760.0)),
                ),
        )
        .child(row().w_full().pb(18.0).justify_center().child(composer))
        .child(node_card(s, th))
}

fn node_card(s: &State, th: &Theme) -> El {
    let Some(index) = s.selected_node else {
        return el();
    };
    let Some(node) = s.graph.nodes.get(index) else {
        return el();
    };
    col()
        .key("node-card")
        .size(360.0, 150.0)
        .padding(14.0)
        .gap(8.0)
        .radius(12.0)
        .fill(th.card)
        .border(1.0, th.border)
        .absolute(20.0, 600.0)
        .child(
            row()
                .w_full()
                .items_center()
                .child(
                    text(node.name.clone())
                        .font(mono(13.0, 500.0))
                        .color(th.fg)
                        .grow(1.0),
                )
                .child(
                    text("✕".to_owned())
                        .font(mono(12.0, 400.0))
                        .color(th.muted)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| s.selected_node = None),
                ),
        )
        .child(
            text(node.statement.clone())
                .font(prose())
                .color(th.muted)
                .w_full(),
        )
        .child(
            row()
                .gap(8.0)
                .child(
                    text("Rename".to_owned())
                        .key("node-rename")
                        .font(mono(11.0, 500.0))
                        .color(th.fg)
                        .padding(6.0)
                        .radius(6.0)
                        .hover(th.hover)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| {
                            if let Some(index) = s.selected_node
                                && let Some(node) = s.graph.nodes.get(index)
                            {
                                s.node_rename_text = TextEdit::new(node.name.clone());
                                s.node_rename_open = true;
                            }
                        }),
                )
                .child(
                    text("Delete".to_owned())
                        .key("node-delete")
                        .font(mono(11.0, 500.0))
                        .color(th.red)
                        .padding(6.0)
                        .radius(6.0)
                        .hover(th.hover)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| s.delete_selected_node()),
                ),
        )
}

fn chains(s: &State, th: &Theme) -> El {
    let mut list = col().w_full().gap(8.0);
    for (i, chain) in s.graph.chains.iter().enumerate() {
        list = list.child(
            col()
                .key(format!("chain-{i}"))
                .w_full()
                .gap(6.0)
                .padding(12.0)
                .radius(8.0)
                .fill(th.card)
                .border(1.0, th.border_soft)
                .child(
                    text(chain.premises.join("  ·  "))
                        .font(mono(11.0, 400.0))
                        .color(th.muted)
                        .w_full(),
                )
                .child(
                    row()
                        .gap(8.0)
                        .items_center()
                        .child(
                            icon(&theme::ARROW_UP, 12.0)
                                .color(th.faint)
                                .rotate(std::f32::consts::PI),
                        )
                        .child(
                            text(chain.conclusion.clone())
                                .font(mono(12.5, 500.0))
                                .color(th.fg),
                        ),
                ),
        );
    }
    let card = col()
        .key("chains-dialog")
        .size(820.0, 600.0)
        .padding(20.0)
        .gap(14.0)
        .radius(16.0)
        .fill(th.bg)
        .border(1.0, th.border)
        .child(
            row()
                .w_full()
                .items_center()
                .child(
                    text("Argument chains".to_owned())
                        .font(serif(22.0, 700.0))
                        .color(th.fg)
                        .grow(1.0),
                )
                .child(
                    text("✕".to_owned())
                        .key("chains-close")
                        .font(mono(14.0, 400.0))
                        .color(th.muted)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| s.chains_open = false),
                ),
        )
        .child(
            col()
                .w_full()
                .grow(1.0)
                .shrink()
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(list),
        );
    el().w_full()
        .h_full()
        .fill(th.bg)
        .center()
        .child(card)
        .on_press(|s: &mut State| s.chains_open = false)
}

fn mode_tab(index: usize, label: &str, glyph: &'static rune::Icon, s: &State, th: &Theme) -> El {
    let selected = s.mode == index;
    row()
        .key(format!("mode-{index}"))
        .h(26.0)
        .px(10.0)
        .gap(6.0)
        .items_center()
        .radius(6.0)
        .fill(if selected {
            th.raised
        } else {
            Color::TRANSPARENT
        })
        .child(icon(glyph, 13.0).color(if selected { th.fg } else { th.muted }))
        .child(
            text(label.to_owned())
                .font(mono(11.0, 500.0))
                .color(if selected { th.fg } else { th.muted }),
        )
        .hover(th.hover)
        .cursor(Cursor::Pointer)
        .on_press(move |s: &mut State| s.mode = index)
}

fn model_pill(s: &State, th: &Theme) -> El {
    let short = s.model.rsplit('/').next().unwrap_or(&s.model);
    let name = if short.chars().count() > 26 {
        let cut: String = short.chars().take(25).collect();
        format!("{cut}…")
    } else {
        short.to_owned()
    };
    row()
        .key("model-pill")
        .w(198.0)
        .h(30.0)
        .px(10.0)
        .gap(6.0)
        .items_center()
        .radius(8.0)
        .fill(th.card)
        .border(1.0, th.border_soft)
        .child(
            text(name)
                .font(mono(11.0, 400.0))
                .color(th.muted)
                .grow(1.0)
                .max_w(160.0),
        )
        .child(icon(&theme::CHEVRON_DOWN, 12.0).color(th.muted))
        .hover(th.hover)
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut State| s.model_menu_open = true)
}

fn session(s: &State, th: &Theme) -> El {
    let (title, rows) = match &s.chat {
        Some(c) => (c.title.clone(), c.rows.clone()),
        None => ("New session".to_owned(), Vec::new()),
    };
    let kind = if s.chat.as_ref().is_some_and(|c| c.kind == "practice") {
        "PRACTICE"
    } else {
        "TEACHING"
    };
    let top = row()
        .w_full()
        .h(64.0)
        .px(28.0)
        .items_center()
        .gap(10.0)
        .child(if s.sidebar {
            el()
        } else {
            icon_button(
                "session-toggle-sidebar",
                &theme::PANEL_LEFT,
                17.0,
                th,
                th.muted,
            )
            .on_press(|s: &mut State| s.sidebar = true)
        })
        .child(text(title).font(serif(36.0, 700.0)).color(th.fg).grow(1.0))
        .child(
            top_pill("plan", &theme::PLAN, "PLAN", th, false)
                .on_press(|s: &mut State| s.plan_open = true),
        )
        .child(top_pill("teaching", &theme::CIRCLE, kind, th, false))
        .window_drag();

    let mut transcript = col().w_full().gap(22.0).py(18.0);
    let last_quiz = rows
        .iter()
        .rposition(|r| matches!(r, Row::Quiz(q) if q.answered.is_none()));
    let last_problem = rows
        .iter()
        .rposition(|r| matches!(r, Row::Problem(p) if p.concluded.is_none()));
    for (i, row_) in rows.iter().enumerate() {
        match row_ {
            Row::User(t) => {
                transcript = transcript.child(
                    row().w_full().justify_end().child(
                        el().max_w(560.0)
                            .px(14.0)
                            .py(10.0)
                            .radius(12.0)
                            .fill(th.card)
                            .border(1.0, th.border_soft)
                            .child(md::para(t, th)),
                    ),
                );
            }
            Row::Assistant(blocks) => {
                transcript = transcript.child(md::blocks(blocks, th));
            }
            Row::Quiz(quiz) => {
                let live = last_quiz == Some(i);
                transcript = transcript.child(quiz_card(quiz, live, s, th));
            }
            Row::Problem(problem) => {
                let live = s.pending_problem && last_problem == Some(i);
                transcript = transcript.child(problem_card(problem, live, s, th));
            }
        }
    }

    if s.busy || s.stream_error.is_some() {
        transcript = transcript.child(col().w_full().gap(8.0).child(
            if let Some(error) = &s.stream_error {
                text(error.clone())
                    .font(mono(12.0, 400.0))
                    .color(th.red)
                    .w_full()
            } else if s.stream_text.trim().is_empty() {
                text("Thinking…".to_owned())
                    .font(mono(12.0, 400.0))
                    .color(th.muted)
            } else {
                md::para(&s.stream_text, th)
            },
        ));
    }

    let scroll = col()
        .key("transcript")
        .w_full()
        .grow(1.0)
        .shrink()
        .min_h(0.0)
        .scroll(ScrollSpec { stick_bottom: true })
        .child(
            row()
                .w_full()
                .px(32.0)
                .child(transcript.max_w(1544.0).grow(1.0)),
        );

    col()
        .key("session")
        .w_full()
        .h_full()
        .child(top)
        .child(scroll)
        .child(reply_bar(s, th))
        .child(stats_bar(s, th))
}

fn plan_window(s: &State, th: &Theme) -> El {
    let Some(plan) = s.chat.as_ref().and_then(|c| c.plan.as_ref()) else {
        return el().w_full().h_full().fill(th.bg).center().child(
            col()
                .gap(10.0)
                .items_center()
                .child(
                    text("No plan yet".to_owned())
                        .font(serif(22.0, 700.0))
                        .color(th.fg),
                )
                .child(
                    text("The teacher has not authored a plan for this session.".to_owned())
                        .font(mono(12.0, 400.0))
                        .color(th.muted),
                )
                .child(
                    text("Close".to_owned())
                        .key("plan-close")
                        .font(mono(12.0, 500.0))
                        .color(th.fg)
                        .padding(8.0)
                        .radius(6.0)
                        .hover(th.hover)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| s.plan_open = false),
                ),
        );
    };
    let mut per_layer: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut pos: Vec<[f32; 2]> = Vec::with_capacity(plan.labels.len());
    let mut width = 0.0f32;
    let mut height = 0.0f32;
    for (i, _) in plan.labels.iter().enumerate() {
        let depth = plan.depth.get(i).copied().unwrap_or(0);
        let row = per_layer.entry(depth).or_insert(0);
        let p = [24.0 + depth as f32 * 150.0, 24.0 + *row as f32 * 72.0];
        *row += 1;
        width = width.max(p[0] + 150.0);
        height = height.max(p[1] + 60.0);
        pos.push(p);
    }
    let mut edges = Shape::new();
    for (a, b) in &plan.edges {
        let from = [pos[*a][0] + 140.0, pos[*a][1] + 28.0];
        let to = [pos[*b][0], pos[*b][1] + 28.0];
        edges = edges.line(
            from,
            to,
            1.0,
            LineCap::Round,
            Color {
                a: 0.35,
                ..th.muted
            },
        );
    }
    let mut canvas = col()
        .key("plan-canvas")
        .size(width.max(600.0), height.max(300.0))
        .child(
            el().size(width.max(600.0), height.max(300.0))
                .absolute(0.0, 0.0)
                .shape(edges)
                .a11y_hidden(),
        );
    for (i, label) in plan.labels.iter().enumerate() {
        canvas = canvas.child(
            el().key(format!("plan-node-{i}"))
                .size(140.0, 56.0)
                .absolute(pos[i][0], pos[i][1])
                .padding(8.0)
                .radius(8.0)
                .fill(th.card)
                .border(1.0, th.border_soft)
                .child(
                    text(label.clone())
                        .font(mono(10.0, 400.0))
                        .color(th.fg)
                        .w_full()
                        .wrap_anywhere(),
                ),
        );
    }
    let card = col()
        .key("plan-dialog")
        .size(1300.0, 720.0)
        .padding(20.0)
        .gap(12.0)
        .radius(16.0)
        .fill(th.bg)
        .border(1.0, th.border)
        .child(
            row()
                .w_full()
                .items_center()
                .child(
                    text("Plan".to_owned())
                        .font(serif(22.0, 700.0))
                        .color(th.fg)
                        .grow(1.0),
                )
                .child(
                    text("Close".to_owned())
                        .key("plan-close")
                        .font(mono(12.0, 500.0))
                        .color(th.muted)
                        .padding(6.0)
                        .radius(6.0)
                        .hover(th.hover)
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| s.plan_open = false),
                ),
        )
        .child(
            el().w_full()
                .grow(1.0)
                .shrink()
                .min_h(0.0)
                .scroll_xy()
                .child(canvas),
        );
    el().w_full().h_full().fill(th.bg).center().child(card)
}

fn reply_bar(s: &State, th: &Theme) -> El {
    let send = el()
        .key("session-send")
        .size(34.0, 34.0)
        .center()
        .radius(9.0)
        .fill(th.raised)
        .child(icon(&theme::ARROW_UP, 17.0).color(th.fg))
        .hover(th.hover)
        .press(th.pressed)
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut State| {
            let text = s.reply.text.clone();
            s.send_message(&text);
        });
    row().w_full().justify_center().py(8.0).child(
        row()
            .w(700.0)
            .h(48.0)
            .px(14.0)
            .gap(8.0)
            .items_center()
            .radius(12.0)
            .fill(th.card)
            .border(1.0, th.border)
            .child(
                text_input(&s.reply, prose())
                    .key("session-reply")
                    .grow(1.0)
                    .shrink()
                    .h(36.0)
                    .pt(6.0)
                    .placeholder("Ask, answer, or steer the lesson…", th.faint)
                    .color(th.fg)
                    .fill(Color::TRANSPARENT)
                    .on_edit(|s: &mut State| &mut s.reply)
                    .on_submit(|s: &mut State, _| {
                        let text = s.reply.text.clone();
                        s.send_message(&text);
                    }),
            )
            .child(send),
    )
}

fn quiz_card(quiz: &data::Quiz, live: bool, s: &State, th: &Theme) -> El {
    let mut card = col()
        .w_full()
        .gap(10.0)
        .padding(16.0)
        .radius(12.0)
        .fill(Color { a: 0.35, ..th.card })
        .border(1.0, th.border)
        .child(
            text("PREDICTION".to_owned())
                .font(mono(10.0, 500.0))
                .color(th.muted),
        )
        .child(md::para(&quiz.question, th));

    for (i, option) in quiz.options.iter().enumerate() {
        let chosen = if live {
            s.quiz_selected == Some(i)
        } else {
            quiz.answered.as_ref().and_then(|a| a.chosen) == Some(i)
        };
        let is_correct = quiz
            .answered
            .as_ref()
            .is_some_and(|a| a.chosen == Some(i) && a.correct == Some(true));
        let border = if chosen { th.fg } else { th.border_soft };
        let fill = if chosen {
            th.raised
        } else {
            Color::TRANSPARENT
        };
        let mut row_ = row()
            .w_full()
            .gap(10.0)
            .items_start()
            .px(12.0)
            .py(11.0)
            .radius(8.0)
            .fill(fill)
            .border(1.0, if is_correct { th.green } else { border })
            .child(
                el().pt(3.0).child(
                    el().size(14.0, 14.0)
                        .radius(7.0)
                        .border(1.5, if chosen { th.fg } else { th.border })
                        .child(if chosen {
                            el().size(6.0, 6.0).radius(3.0).fill(th.fg).center()
                        } else {
                            el()
                        }),
                ),
            )
            .child(md::para(option, th).grow(1.0));
        if live {
            row_ = row_
                .key(format!("option-{i}"))
                .hover(th.hover)
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    s.quiz_selected = Some(i);
                });
        }
        card = card.child(row_);
    }

    if !live {
        if let Some(answered) = &quiz.answered {
            let (label, color) = match answered.correct {
                Some(true) => ("Correct", th.green),
                Some(false) => ("Incorrect", th.red),
                None => ("Answered", th.muted),
            };
            return row()
                .w_full()
                .key("answered-quiz")
                .gap(12.0)
                .items_center()
                .px(14.0)
                .py(10.0)
                .radius(8.0)
                .fill(th.card)
                .border(1.0, th.border_soft)
                .child(dot(th, color))
                .child(md::para(&quiz.question, th).grow(1.0))
                .child(text(label.to_owned()).font(mono(11.0, 500.0)).color(color));
        }
        return row().w_full().child(card);
    }

    // "I don't know"
    let unknown_selected = live && s.quiz_selected == Some(quiz.options.len());
    let mut unknown_row = row()
        .w_full()
        .gap(10.0)
        .items_start()
        .px(12.0)
        .py(11.0)
        .radius(8.0)
        .border_dash(3.0, 3.0)
        .border(1.0, th.border_soft)
        .child(
            el().pt(3.0).child(
                el().size(14.0, 14.0)
                    .radius(7.0)
                    .border(1.5, if unknown_selected { th.fg } else { th.border })
                    .child(if unknown_selected {
                        el().size(6.0, 6.0).radius(3.0).fill(th.fg).center()
                    } else {
                        el()
                    }),
            ),
        )
        .child(
            text("I don't know".to_owned())
                .font(prose())
                .color(th.muted)
                .grow(1.0),
        );
    if live {
        unknown_row = unknown_row
            .key("option-unknown")
            .hover(th.hover)
            .cursor(Cursor::Pointer)
            .on_press(|s: &mut State| {
                s.quiz_selected = Some(usize::MAX);
            });
    }
    card = card.child(unknown_row);

    // confidence
    let pct = (s.confidence * 100.0).round() as i32;
    let track_w = 540.0;
    card = card.child(
        row()
            .w_full()
            .gap(12.0)
            .items_center()
            .child(
                text("CONFIDENCE".to_owned())
                    .font(mono(10.0, 500.0))
                    .color(th.muted)
                    .w(96.0),
            )
            .child(
                el().key("confidence-slider")
                    .size(track_w, 18.0)
                    .child(el().absolute(0.0, 8.0).size(track_w, 2.0).fill(th.border))
                    .child(
                        el().absolute(s.confidence * (track_w - 14.0), 2.0)
                            .size(14.0, 14.0)
                            .radius(7.0)
                            .fill(th.gold),
                    )
                    .cursor(Cursor::Pointer)
                    .on_drag(|s: &mut State, d: &mut rune::Drag, _| {
                        if d.w > 0.0 {
                            s.confidence = ((d.x / d.w).clamp(0.0, 1.0) * 10.0).round() / 10.0;
                        }
                    }),
            )
            .child(text(format!("{pct}%")).font(mono(11.0, 400.0)).color(th.fg)),
    );

    // thinking
    card = card.child(
        col()
            .w_full()
            .gap(6.0)
            .child(
                text("YOUR THINKING".to_owned())
                    .font(mono(10.0, 500.0))
                    .color(th.muted),
            )
            .child(
                text_input(&s.thinking, prose())
                    .key("quiz-thinking")
                    .min_h(74.0)
                    .padding(10.0)
                    .placeholder("Why this answer? Whatever comes to mind.", th.faint)
                    .color(th.fg)
                    .fill(Color::TRANSPARENT)
                    .border(1.0, th.border_soft)
                    .radius(8.0)
                    .on_edit(|s: &mut State| &mut s.thinking),
            ),
    );

    let submit = el()
        .key("submit-answer")
        .h(40.0)
        .px(18.0)
        .center()
        .radius(8.0)
        .fill(th.fg)
        .child(
            text("Submit answer".to_owned())
                .font(mono(12.5, 500.0))
                .color(th.bg),
        )
        .hover(Color { a: 0.85, ..th.fg })
        .press(Color { a: 0.75, ..th.fg })
        .cursor(Cursor::Pointer)
        .on_press(|s: &mut State| s.submit_quiz());

    card = card.child(
        row()
            .w_full()
            .items_center()
            .child(
                text("Enter submits · Tab newline".to_owned())
                    .font(mono(11.0, 400.0))
                    .color(th.faint)
                    .grow(1.0),
            )
            .child(if live { submit } else { el() }),
    );

    row().w_full().justify_center().child(card)
}

fn problem_card(problem: &data::Problem, live: bool, s: &State, th: &Theme) -> El {
    let mut card = col()
        .w_full()
        .max_w(740.0)
        .gap(10.0)
        .padding(16.0)
        .radius(12.0)
        .fill(Color { a: 0.35, ..th.card })
        .border(1.0, th.border)
        .child(
            text(format!("PRACTICE · {}", problem.mode.to_uppercase()))
                .font(mono(10.0, 500.0))
                .color(th.muted),
        )
        .child(md::blocks(&data::parse_markdown(&problem.statement), th));
    if let Some(done) = &problem.concluded {
        card = card
            .child(
                text(format!("RESULT · {}", done.outcome.to_uppercase()))
                    .font(mono(10.0, 500.0))
                    .color(th.muted),
            )
            .child(md::para(&done.verdict, th))
            .child(
                text("MODEL SOLUTION".to_owned())
                    .font(mono(10.0, 500.0))
                    .color(th.muted),
            )
            .child(md::blocks(&data::parse_markdown(&problem.solution), th));
    } else if live {
        card = card
            .child(
                col()
                    .w_full()
                    .gap(6.0)
                    .child(
                        text("YOUR ANSWER".to_owned())
                            .font(mono(10.0, 500.0))
                            .color(th.muted),
                    )
                    .child(
                        text_input(&s.practice_answer, prose())
                            .key("practice-answer")
                            .min_h(86.0)
                            .padding(10.0)
                            .placeholder("Work it out here…", th.faint)
                            .color(th.fg)
                            .fill(Color::TRANSPARENT)
                            .border(1.0, th.border_soft)
                            .radius(8.0)
                            .on_edit(|s: &mut State| &mut s.practice_answer),
                    ),
            )
            .child(
                row().w_full().justify_end().child(
                    el().key("submit-problem")
                        .h(40.0)
                        .px(18.0)
                        .center()
                        .radius(8.0)
                        .fill(th.fg)
                        .child(
                            text("Submit answer".to_owned())
                                .font(mono(12.5, 500.0))
                                .color(th.bg),
                        )
                        .hover(Color { a: 0.85, ..th.fg })
                        .press(Color { a: 0.75, ..th.fg })
                        .cursor(Cursor::Pointer)
                        .on_press(|s: &mut State| s.submit_problem()),
                ),
            );
    }
    row().w_full().justify_center().child(card)
}

fn modal_card(
    key: &str,
    width: f32,
    height: f32,
    title: &str,
    th: &Theme,
    close: fn(&mut State),
) -> (El, El) {
    let card = col()
        .key(key)
        .size(width, height)
        .padding(20.0)
        .gap(12.0)
        .radius(16.0)
        .fill(th.bg)
        .border(1.0, th.border);
    let header = row()
        .w_full()
        .items_center()
        .child(
            text(title.to_owned())
                .font(serif(22.0, 700.0))
                .color(th.fg)
                .grow(1.0),
        )
        .child(
            text("Close".to_owned())
                .key("modal-close")
                .font(mono(12.0, 500.0))
                .color(th.muted)
                .padding(6.0)
                .radius(6.0)
                .hover(th.hover)
                .cursor(Cursor::Pointer)
                .on_press(close),
        );
    (card, header)
}

fn model_menu(s: &State, th: &Theme) -> El {
    let rows = s.models.len().max(1) as f32;
    let (card, header) = modal_card(
        "model-menu",
        560.0,
        120.0 + rows * 48.0,
        "Model",
        th,
        |s: &mut State| s.model_menu_open = false,
    );
    let mut card = card.child(header);
    if s.models.is_empty() {
        card = card.child(
            text("No models configured".to_owned())
                .font(mono(12.0, 400.0))
                .color(th.muted),
        );
    }
    for (i, (label, id)) in s.models.iter().enumerate() {
        let selected = s.model == *id;
        let value = id.clone();
        card = card.child(
            row()
                .key(format!("model-choice-{i}"))
                .w_full()
                .h(42.0)
                .px(12.0)
                .gap(10.0)
                .items_center()
                .radius(8.0)
                .fill(if selected {
                    th.raised
                } else {
                    Color::TRANSPARENT
                })
                .border(
                    1.0,
                    if selected {
                        th.border
                    } else {
                        Color::TRANSPARENT
                    },
                )
                .child(if selected {
                    icon(&theme::CHECK, 14.0).color(th.green)
                } else {
                    el().size(14.0, 14.0)
                })
                .child(
                    text(label.clone())
                        .font(mono(11.0, 500.0))
                        .color(th.muted)
                        .w(52.0),
                )
                .child(
                    text(id.clone())
                        .font(mono(11.0, 400.0))
                        .color(th.fg)
                        .grow(1.0),
                )
                .hover(th.hover)
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    s.model = value.clone();
                    data::set_strong_model(&value);
                    s.model_menu_open = false;
                }),
        );
    }
    el().w_full().h_full().fill(th.bg).center().child(card)
}

fn effort_menu(s: &State, th: &Theme) -> El {
    let (card, header) = modal_card(
        "effort-menu",
        520.0,
        120.0 + 3.0 * 56.0,
        "Thinking effort",
        th,
        |s: &mut State| s.effort_menu_open = false,
    );
    let mut card = card.child(header);
    for (i, (value, desc)) in [
        ("max", "Deepest reasoning; slower turns."),
        ("medium", "Balanced reasoning and speed."),
        ("minimal", "Fastest replies."),
    ]
    .iter()
    .enumerate()
    {
        let selected = s.effort == *value;
        let value = (*value).to_owned();
        card = card.child(
            row()
                .key(format!("effort-choice-{i}"))
                .w_full()
                .h(50.0)
                .px(12.0)
                .gap(10.0)
                .items_center()
                .radius(8.0)
                .fill(if selected {
                    th.raised
                } else {
                    Color::TRANSPARENT
                })
                .border(
                    1.0,
                    if selected {
                        th.border
                    } else {
                        Color::TRANSPARENT
                    },
                )
                .child(if selected {
                    icon(&theme::CHECK, 14.0).color(th.green)
                } else {
                    el().size(14.0, 14.0)
                })
                .child(
                    col()
                        .gap(2.0)
                        .grow(1.0)
                        .child(text(value.clone()).font(mono(12.0, 500.0)).color(th.fg))
                        .child(
                            text((*desc).to_owned())
                                .font(mono(10.5, 400.0))
                                .color(th.muted),
                        ),
                )
                .hover(th.hover)
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| {
                    s.effort = value.clone();
                    data::set_effort(&value);
                    s.effort_menu_open = false;
                }),
        );
    }
    el().w_full().h_full().fill(th.bg).center().child(card)
}

fn dictate_modal(_s: &State, th: &Theme) -> El {
    let (card, header) = modal_card(
        "dictate-dialog",
        520.0,
        176.0,
        "Dictation",
        th,
        |s: &mut State| s.dictate_open = false,
    );
    let card = card
        .child(header)
        .child(
            text("Dictation needs a Deepgram key and microphone capture, which are not wired in this build yet.".to_owned())
                .font(prose())
                .color(th.muted)
                .w_full()
                .wrap(),
        );
    el().w_full().h_full().fill(th.bg).center().child(card)
}

fn node_rename_modal(s: &State, th: &Theme) -> El {
    let (card, header) = modal_card(
        "rename-node-dialog",
        520.0,
        220.0,
        "Rename node",
        th,
        |s: &mut State| s.node_rename_open = false,
    );
    let card = card
        .child(header)
        .child(
            text_input(&s.node_rename_text, mono(13.0, 400.0))
                .key("node-rename-input")
                .single_line()
                .w_full()
                .h(36.0)
                .px(10.0)
                .pt(7.0)
                .color(th.fg)
                .fill(th.card)
                .border(1.0, th.border)
                .radius(8.0)
                .on_edit(|s: &mut State| &mut s.node_rename_text),
        )
        .child(
            row().w_full().justify_end().child(
                text("Save".to_owned())
                    .key("node-rename-save")
                    .font(mono(12.0, 500.0))
                    .color(th.bg)
                    .padding(8.0)
                    .radius(6.0)
                    .fill(th.fg)
                    .hover(Color { a: 0.85, ..th.fg })
                    .cursor(Cursor::Pointer)
                    .on_press(|s: &mut State| {
                        if let Some(index) = s.selected_node
                            && let Some(node) = s.graph.nodes.get_mut(index)
                        {
                            let name = s.node_rename_text.text.trim().to_owned();
                            if !name.is_empty() {
                                node.name = name;
                            }
                        }
                        s.node_rename_open = false;
                    }),
            ),
        );
    el().w_full().h_full().fill(th.bg).center().child(card)
}

fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}M", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

fn stats_bar(s: &State, th: &Theme) -> El {
    row().w_full().h(58.0).justify_center().child(
        col()
            .w(700.0)
            .gap(0.0)
            .child(
                row()
                    .w_full()
                    .h(18.0)
                    .items_center()
                    .gap(10.0)
                    .child(
                        text(format!("{} / 202.8k · {}", format_tokens(s.tokens), s.cost))
                            .font(mono(11.0, 400.0))
                            .color(th.muted),
                    )
                    .child(if s.busy {
                        row()
                            .h(20.0)
                            .px(8.0)
                            .gap(5.0)
                            .items_center()
                            .radius(6.0)
                            .fill(th.card)
                            .border(1.0, th.border_soft)
                            .child(icon(&theme::STOP, 11.0).color(th.red))
                            .child(text("Stop".to_owned()).font(mono(11.0, 500.0)).color(th.fg))
                            .hover(th.hover)
                            .cursor(Cursor::Pointer)
                            .on_press(|s: &mut State| s.stop_stream())
                    } else {
                        el()
                    }),
            )
            .child(
                row()
                    .w_full()
                    .h(32.0)
                    .gap(10.0)
                    .items_center()
                    .justify_end()
                    .child(
                        row()
                            .key("dictate")
                            .h(32.0)
                            .w(218.0)
                            .items_center()
                            .justify_center()
                            .gap(7.0)
                            .radius(8.0)
                            .fill(th.card)
                            .border(1.0, th.border_soft)
                            .child(icon(&theme::MIC, 14.0).color(th.muted))
                            .child(
                                text("Dictate".to_owned())
                                    .font(mono(11.0, 400.0))
                                    .color(th.muted),
                            )
                            .hover(th.hover)
                            .cursor(Cursor::Pointer)
                            .on_press(|s: &mut State| s.dictate_open = true),
                    )
                    .child(model_pill(s, th))
                    .child(
                        top_pill("effort-session", &theme::CHEVRON_DOWN, &s.effort, th, true)
                            .on_press(|s: &mut State| s.effort_menu_open = true),
                    ),
            ),
    )
}

fn settings(s: &State, th: &Theme) -> El {
    let sections = [
        "General",
        "Providers",
        "Models",
        "Appearance",
        "Graph",
        "Prompts",
        "Keyboard",
    ];
    let mut nav = col().w(180.0).gap(2.0);
    for (i, name) in sections.iter().enumerate() {
        let selected = s.settings_section == i;
        nav = nav.child(
            row()
                .key(format!("settings-nav-{i}"))
                .w_full()
                .h(32.0)
                .px(10.0)
                .items_center()
                .radius(6.0)
                .fill(if selected {
                    th.raised
                } else {
                    Color::TRANSPARENT
                })
                .child(
                    text((*name).to_owned())
                        .font(mono(12.0, 500.0))
                        .color(if selected { th.fg } else { th.muted }),
                )
                .hover(th.hover)
                .cursor(Cursor::Pointer)
                .on_press(move |s: &mut State| s.settings_section = i),
        );
    }

    let mut body = col().w_full().gap(14.0);
    body = body
        .child(
            text(sections[s.settings_section].to_owned())
                .font(section_title())
                .color(th.fg),
        )
        .child(el().w_full().h(1.0).fill(th.border_soft));
    let is_dark = s.dark_override.unwrap_or(th.dark);
    match s.settings_section {
        3 => {
            body = body
                .child(
                    text("Appearance".to_owned())
                        .font(mono(12.0, 500.0))
                        .color(th.muted),
                )
                .child(
                    row()
                        .gap(10.0)
                        .child(
                            row()
                                .key("theme-dark")
                                .h(40.0)
                                .px(14.0)
                                .gap(8.0)
                                .items_center()
                                .radius(8.0)
                                .fill(th.card)
                                .border(
                                    if is_dark { 2.0 } else { 1.0 },
                                    if is_dark { th.fg } else { th.border_soft },
                                )
                                .child(dot(th, theme::theme(true).bg))
                                .child(
                                    text("Dark".to_owned())
                                        .font(mono(12.0, 400.0))
                                        .color(if is_dark { th.fg } else { th.muted }),
                                )
                                .cursor(Cursor::Pointer)
                                .on_press(|s: &mut State| {
                                    s.dark_override = Some(true);
                                    data::set_appearance(true);
                                }),
                        )
                        .child(
                            row()
                                .key("theme-light")
                                .h(40.0)
                                .px(14.0)
                                .gap(8.0)
                                .items_center()
                                .radius(8.0)
                                .fill(th.card)
                                .border(
                                    if is_dark { 1.0 } else { 2.0 },
                                    if is_dark { th.border_soft } else { th.fg },
                                )
                                .child(dot(
                                    th,
                                    Color {
                                        r: 0.96,
                                        g: 0.96,
                                        b: 0.97,
                                        a: 1.0,
                                    },
                                ))
                                .child(
                                    text("Light".to_owned())
                                        .font(mono(12.0, 400.0))
                                        .color(if is_dark { th.muted } else { th.fg }),
                                )
                                .cursor(Cursor::Pointer)
                                .on_press(|s: &mut State| {
                                    s.dark_override = Some(false);
                                    data::set_appearance(false);
                                }),
                        ),
                );
        }
        2 => {
            body = body
                .child(setting_row("Strong model", &s.model, th))
                .child(setting_row(
                    "Cheap model",
                    "nvidia/nemotron-3.5-lightning:free",
                    th,
                ))
                .child(setting_row(
                    "Vision model",
                    "anthropic/claude-haiku-5.5",
                    th,
                ));
        }
        1 => {
            body = body
                .child(setting_row("OpenRouter", "Connection configured", th))
                .child(setting_row("OpenAI", "Not connected", th))
                .child(setting_row("Anthropic", "Not connected", th));
        }
        4 => {
            body = body
                .child(setting_row("Graph rendering", "On", th))
                .child(setting_row(
                    "Propositions",
                    &format!("{} nodes", s.graph.nodes.len()),
                    th,
                ))
                .child(setting_row(
                    "Argument chains",
                    &format!("{} chains", s.graph.chains.len()),
                    th,
                ));
        }
        5 => {
            body = body
                .child(setting_row("Teaching style", "Guided", th))
                .child(setting_row("Shared prompts", "20-question-tools", th))
                .child(setting_row("Custom overrides", "None", th));
        }
        6 => {
            body = body
                .child(setting_row("Submit", "Enter", th))
                .child(setting_row("New line", "Shift+Enter", th))
                .child(setting_row("Close dialog", "Esc", th))
                .child(setting_row("Focus search", "/", th));
        }
        _ => {
            body = body
                .child(setting_row("Auto update", "Off", th))
                .child(setting_row("Dictation language", "English", th))
                .child(setting_row("Data folder", "LearnHarness", th));
        }
    }

    let card = row()
        .key("settings-dialog")
        .size(960.0, 620.0)
        .gap(0.0)
        .radius(16.0)
        .fill(th.bg)
        .border(1.0, th.border)
        .child(
            col()
                .w(190.0)
                .h_full()
                .padding(14.0)
                .gap(10.0)
                .border_sides([0.0, 1.0, 0.0, 0.0], th.border_soft)
                .child(
                    row()
                        .w_full()
                        .items_center()
                        .gap(8.0)
                        .child(
                            text("Settings".to_owned())
                                .font(serif(20.0, 700.0))
                                .color(th.fg),
                        )
                        .child(el().grow(1.0))
                        .child(
                            text("✕".to_owned())
                                .key("settings-close")
                                .font(mono(14.0, 400.0))
                                .color(th.muted)
                                .cursor(Cursor::Pointer)
                                .on_press(|s: &mut State| s.settings_open = false),
                        ),
                )
                .child(nav),
        )
        .child(col().grow(1.0).h_full().padding(24.0).child(body));

    el().key("settings-scrim")
        .w_full()
        .h_full()
        .fill(Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.55,
        })
        .center()
        .child(card)
        .on_press(|s: &mut State| s.settings_open = false)
}

fn setting_row(label: &str, value: &str, th: &Theme) -> El {
    row()
        .w_full()
        .py(10.0)
        .gap(12.0)
        .items_center()
        .border_sides([0.0, 0.0, 1.0, 0.0], th.border_soft)
        .child(
            text(label.to_owned())
                .font(mono(12.0, 500.0))
                .color(th.fg)
                .w(200.0),
        )
        .child(
            text(value.to_owned())
                .font(mono(12.0, 400.0))
                .color(th.muted)
                .grow(1.0),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn practice_events_pose_answer_and_reveal() {
        let mut state = State::new();
        state.active_id = Some("missing_practice_chat".to_owned());
        state.chat = Some(Chat {
            id: "missing_practice_chat".to_owned(),
            title: "practice".to_owned(),
            kind: "practice".to_owned(),
            rows: vec![Row::User("start".to_owned())],
            plan: None,
        });
        let (inbox, inputs) = channel();
        let (events, event_rx) = channel();
        state.agent = Some(ai::AgentHandle {
            inbox,
            events: event_rx,
        });

        events
            .send(brainroom_agent::Event::Problem {
                call_id: "prob-1".to_owned(),
                statement: "Differentiate x^2.".to_owned(),
                mode: "recall".to_owned(),
                solution: "2x".to_owned(),
            })
            .unwrap();
        assert!(state.poll());
        assert!(state.pending_problem);
        assert!(matches!(
            state.chat.as_ref().unwrap().rows.last(),
            Some(Row::Problem(_))
        ));

        state.practice_answer = TextEdit::new("2x");
        state.submit_problem();
        match inputs
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap()
        {
            brainroom_agent::Input::ToolResult { call_id, content } => {
                assert_eq!(call_id, "prob-1");
                assert!(content.contains("2x"));
            }
            other => panic!("expected tool result, got {other:?}"),
        }

        events
            .send(brainroom_agent::Event::Concluded {
                outcome: "solved".to_owned(),
                verdict: "Correct.".to_owned(),
                solution: "2x".to_owned(),
            })
            .unwrap();
        assert!(state.poll());
        let problem = state
            .chat
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .find_map(|row| match row {
                Row::Problem(p) => Some(p),
                _ => None,
            })
            .expect("problem row");
        let concluded = problem.concluded.as_ref().expect("concluded");
        assert_eq!(concluded.outcome, "solved");
        assert_eq!(concluded.verdict, "Correct.");
    }

    #[test]
    fn agent_events_create_a_quiz_and_answer_returns_the_tool_result() {
        let mut state = State::new();
        state.active_id = Some("missing_test_chat".to_owned());
        state.chat = Some(Chat {
            id: "missing_test_chat".to_owned(),
            title: "test".to_owned(),
            kind: "teaching".to_owned(),
            rows: vec![Row::User("start".to_owned())],
            plan: None,
        });
        let (inbox, inputs) = channel();
        let (events, event_rx) = channel();
        state.agent = Some(ai::AgentHandle {
            inbox,
            events: event_rx,
        });

        events
            .send(brainroom_agent::Event::Text("Try this.".to_owned()))
            .unwrap();
        events
            .send(brainroom_agent::Event::Usage(brainroom_agent::Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cost: Some(0.25),
            }))
            .unwrap();
        assert!(state.poll());
        assert_eq!(state.tokens, 15);
        assert_eq!(state.cost, "$0.2500");
        events
            .send(brainroom_agent::Event::Quiz {
                call_id: "call-1".to_owned(),
                question: "What is 2+2?".to_owned(),
                options: vec!["3".to_owned(), "4".to_owned()],
                explanation: None,
            })
            .unwrap();
        assert!(state.poll());
        assert_eq!(state.stream_text, "Try this.");
        let quiz = state
            .chat
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .find_map(|row| match row {
                Row::Quiz(q) => Some(q),
                _ => None,
            })
            .expect("quiz row");
        assert_eq!(quiz.question, "What is 2+2?");
        assert_eq!(state.pending_call.as_deref(), Some("call-1"));

        state.quiz_selected = Some(1);
        state.confidence = 0.7;
        state.submit_quiz();
        let input = inputs
            .recv_timeout(std::time::Duration::from_secs(1))
            .unwrap();
        match input {
            brainroom_agent::Input::ToolResult { call_id, content } => {
                assert_eq!(call_id, "call-1");
                assert!(content.contains("4"));
                assert!(content.contains("70%"));
            }
            other => panic!("expected tool result, got {other:?}"),
        }
    }
}
