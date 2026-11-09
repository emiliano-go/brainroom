//! LearnHarness data: chats, transcripts and the knowledge graph.

use std::collections::HashMap;
use std::path::PathBuf;

use brainroom_store::Store;
use serde_json::Value;

#[derive(Clone, Debug)]
pub enum Block {
    Para(String),
    Heading(u8, String),
    Display(String),
    Code(String),
    Bullet(Vec<String>),
    Numbered(Vec<(u32, String)>),
    Table(Vec<Vec<String>>),
    Quote(String),
    Rule,
}

#[derive(Clone, Debug)]
pub struct Quiz {
    pub question: String,
    pub options: Vec<String>,
    pub answered: Option<Answered>,
    pub call_id: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Answered {
    pub chosen: Option<usize>,
    pub correct: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct Concluded {
    pub outcome: String,
    pub verdict: String,
}

#[derive(Clone, Debug)]
pub struct Problem {
    pub statement: String,
    pub mode: String,
    pub solution: String,
    pub concluded: Option<Concluded>,
}

#[derive(Clone, Debug)]
pub enum Row {
    User(String),
    Assistant(Vec<Block>),
    Quiz(Quiz),
    Problem(Problem),
}

#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub labels: Vec<String>,
    pub edges: Vec<(usize, usize)>,
    pub depth: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Chat {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub rows: Vec<Row>,
    pub plan: Option<Plan>,
}

#[derive(Clone, Debug)]
pub struct SessionRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub kind: String,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    pub statement: String,
    pub x: f32,
    pub y: f32,
    pub chat: String,
}

#[derive(Clone, Debug)]
pub struct Chain {
    pub premises: Vec<String>,
    pub conclusion: String,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<(usize, usize)>,
    pub clusters: Vec<(String, f32, f32)>,
    pub chains: Vec<Chain>,
}

pub fn settings_value() -> Option<Value> {
    let text = std::fs::read_to_string(data_dir().join("settings.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_settings(value: &Value) -> bool {
    let path = data_dir().join("settings.json");
    serde_json::to_string_pretty(value)
        .ok()
        .and_then(|text| std::fs::write(path, text).ok())
        .is_some()
}

/// (label, model id) for the three configured slots.
pub fn model_slots() -> Vec<(String, String)> {
    let Some(value) = settings_value() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (label, key) in [
        ("Strong", "strong"),
        ("Cheap", "cheap"),
        ("Vision", "vision"),
    ] {
        if let Some(id) = value
            .pointer(&format!("/models/{key}/model/id"))
            .and_then(Value::as_str)
        {
            out.push((label.to_owned(), id.to_owned()));
        }
    }
    out
}

pub fn set_strong_model(id: &str) -> bool {
    let Some(mut value) = settings_value() else {
        return false;
    };
    let Some(models) = value.get_mut("models").and_then(Value::as_object_mut) else {
        return false;
    };
    let slot = models
        .entry("strong".to_owned())
        .or_insert_with(|| serde_json::json!({}));
    if let Some(slot) = slot.as_object_mut() {
        let model = slot
            .entry("model".to_owned())
            .or_insert_with(|| serde_json::json!({}));
        if let Some(model) = model.as_object_mut() {
            model.insert("id".to_owned(), Value::String(id.to_owned()));
            if !model.contains_key("provider") {
                model.insert(
                    "provider".to_owned(),
                    Value::String("openrouter".to_owned()),
                );
            }
        }
    }
    write_settings(&value)
}

pub fn appearance_dark() -> Option<bool> {
    settings_value().and_then(|v| {
        v.pointer("/appearance/theme")
            .and_then(Value::as_str)
            .map(|t| t == "dark")
    })
}

pub fn set_appearance(dark: bool) -> bool {
    let Some(mut value) = settings_value() else {
        return false;
    };
    if !value.get("appearance").is_some_and(Value::is_object) {
        value["appearance"] = serde_json::json!({});
    }
    value["appearance"]["theme"] = Value::String(if dark { "dark" } else { "light" }.to_owned());
    write_settings(&value)
}

pub fn effort() -> String {
    settings_value()
        .and_then(|v| {
            v.pointer("/agent/thinkingLevel")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "medium".to_owned())
}

pub fn set_effort(level: &str) -> bool {
    let Some(mut value) = settings_value() else {
        return false;
    };
    if !value.get("agent").is_some_and(Value::is_object) {
        value["agent"] = serde_json::json!({});
    }
    value["agent"]["thinkingLevel"] = Value::String(level.to_owned());
    write_settings(&value)
}

pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BRAINROOM_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Some(dir) = std::env::var_os("LH_DATA_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let lh = home.join(".config/LearnHarness/data");
    if lh.exists() {
        lh
    } else {
        home.join(".config/Brainroom/data")
    }
}

pub fn store() -> Store {
    Store::new(data_dir())
}

fn short_date(iso: &str) -> String {
    if iso.len() >= 10 {
        iso[..10].to_owned()
    } else {
        iso.to_owned()
    }
}

fn session_date(iso: &str) -> String {
    let Ok(t) = time::OffsetDateTime::parse(iso, &time::format_description::well_known::Rfc3339)
    else {
        return short_date(iso);
    };
    let secs = (time::OffsetDateTime::now_utc() - t).whole_seconds();
    match secs {
        s if (0..3600).contains(&s) => format!("{}m ago", (s / 60).max(1)),
        s if (3600..86_400).contains(&s) => format!("{}h ago", s / 3600),
        _ => short_date(iso),
    }
}

pub fn sessions(store: &Store) -> Vec<SessionRow> {
    store
        .list_chats()
        .into_iter()
        .map(|c| {
            let kind = std::fs::read_to_string(store.chats_dir().join(format!("{}.json", c.id)))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v.get("kind").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_else(|| "teaching".to_owned());
            SessionRow {
                status: format!("Waiting for input · {}", session_date(&c.updated_at)),
                id: c.id,
                title: c.title,
                kind,
            }
        })
        .collect()
}

pub fn parse_markdown(text: &str) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut para: Vec<&str> = Vec::new();
    let mut lines = text.lines().peekable();
    let flush = |para: &mut Vec<&str>, blocks: &mut Vec<Block>| {
        if !para.is_empty() {
            blocks.push(Block::Para(para.join(" ")));
            para.clear();
        }
    };
    while let Some(line) = lines.next() {
        let t = line.trim_end();
        if t.trim().is_empty() {
            flush(&mut para, &mut blocks);
            continue;
        }
        if let Some(rest) = t.strip_prefix("$$") {
            flush(&mut para, &mut blocks);
            let mut tex = String::new();
            if let Some(end) = rest.find("$$") {
                tex.push_str(&rest[..end]);
            } else {
                tex.push_str(rest);
                for l in lines.by_ref() {
                    if let Some(end) = l.find("$$") {
                        if !tex.is_empty() {
                            tex.push('\n');
                        }
                        tex.push_str(&l[..end]);
                        break;
                    }
                    if !tex.is_empty() {
                        tex.push('\n');
                    }
                    tex.push_str(l);
                }
            }
            blocks.push(Block::Display(tex.trim().to_owned()));
            continue;
        }
        let tr = t.trim();
        if tr.starts_with("```") {
            flush(&mut para, &mut blocks);
            let mut code = Vec::new();
            for l in lines.by_ref() {
                if l.trim().starts_with("```") {
                    break;
                }
                code.push(l);
            }
            blocks.push(Block::Code(code.join("\n")));
            continue;
        }
        if let Some(rest) = tr.strip_prefix('#') {
            flush(&mut para, &mut blocks);
            let level = tr.chars().take_while(|c| *c == '#').count().min(6) as u8;
            blocks.push(Block::Heading(
                level,
                rest.trim_start_matches('#').trim().to_owned(),
            ));
            continue;
        }
        if tr == "---" || tr == "***" {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Rule);
            continue;
        }
        if let Some(rest) = tr.strip_prefix("> ") {
            flush(&mut para, &mut blocks);
            blocks.push(Block::Quote(rest.to_owned()));
            continue;
        }
        if tr.starts_with("- ") || tr.starts_with("* ") {
            flush(&mut para, &mut blocks);
            let mut items = vec![tr[2..].to_owned()];
            while let Some(n) = lines.peek() {
                let n = n.trim();
                if let Some(item) = n.strip_prefix("- ").or_else(|| n.strip_prefix("* ")) {
                    items.push(item.to_owned());
                    lines.next();
                } else {
                    break;
                }
            }
            blocks.push(Block::Bullet(items));
            continue;
        }
        if let Some((n, first)) = numbered_item(tr) {
            flush(&mut para, &mut blocks);
            let mut items = vec![(n, first.to_owned())];
            while let Some(line) = lines.peek() {
                if let Some((n, item)) = numbered_item(line.trim()) {
                    items.push((n, item.to_owned()));
                    lines.next();
                } else {
                    break;
                }
            }
            blocks.push(Block::Numbered(items));
            continue;
        }
        if is_table_row(tr) {
            flush(&mut para, &mut blocks);
            let mut rows = Vec::new();
            rows.push(table_cells(tr));
            let mut seen_separator = false;
            while let Some(n) = lines.peek() {
                let n = n.trim();
                if !is_table_row(n) {
                    break;
                }
                lines.next();
                if !seen_separator && is_table_separator(n) {
                    seen_separator = true;
                    continue;
                }
                rows.push(table_cells(n));
            }
            if rows.len() >= 2 {
                blocks.push(Block::Table(rows));
            } else if let Some(row) = rows.into_iter().next() {
                blocks.push(Block::Para(row.join(" | ")));
            }
            continue;
        }
        para.push(tr);
    }
    flush(&mut para, &mut blocks);
    blocks
}

fn numbered_item(line: &str) -> Option<(u32, &str)> {
    let rest = line.trim_start();
    let dot = rest.find('.')?;
    if dot == 0 {
        return None;
    }
    let n: u32 = rest[..dot].parse().ok()?;
    rest.get(dot + 1..)
        .map(str::trim_start)
        .filter(|s| !s.is_empty())
        .map(|s| (n, s))
}

fn is_table_row(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.ends_with('|') && t.len() > 2
}

fn is_table_separator(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.ends_with('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

fn table_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_owned())
        .collect()
}

fn content_parts(value: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    match value {
        Value::String(s) => out.push(("text".into(), Value::String(s.clone()))),
        Value::Array(parts) => {
            for p in parts {
                let kind = p
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("text")
                    .to_owned();
                out.push((kind, p.clone()));
            }
        }
        _ => {}
    }
    out
}

fn quiz_from_call(call: &Value) -> Option<Quiz> {
    let args = call.get("arguments")?;
    let question = args
        .get("question")
        .and_then(Value::as_str)
        .or_else(|| args.get("explanation").and_then(Value::as_str))
        .unwrap_or_default()
        .to_owned();
    if question.is_empty() {
        return None;
    }
    let options: Vec<String> = args
        .get("options")
        .and_then(Value::as_array)
        .map(|opts| {
            opts.iter()
                .filter_map(|o| o.get("label").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    if options.is_empty() {
        return None;
    }
    Some(Quiz {
        question,
        options,
        answered: None,
        call_id: call.get("id").and_then(Value::as_str).map(str::to_owned),
    })
}

fn apply_result(quiz: &mut Quiz, text: &str) {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return;
    };
    let chosen = v
        .get("answers")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|a| a.get("index"))
        .and_then(Value::as_u64)
        .map(|i| i as usize);
    let correct = v.get("correct").and_then(Value::as_bool);
    if v.get("dontKnow").and_then(Value::as_bool).unwrap_or(false) {
        quiz.answered = Some(Answered {
            chosen: None,
            correct: Some(false),
        });
    } else if chosen.is_some() || correct.is_some() {
        quiz.answered = Some(Answered { chosen, correct });
    }
}

pub fn blocks_text(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        let piece = match block {
            Block::Para(s) | Block::Quote(s) => s.clone(),
            Block::Heading(_, s) => s.clone(),
            Block::Display(tex) => format!("$${tex}$$"),
            Block::Code(s) => s.clone(),
            Block::Bullet(items) => items.join("\n"),
            Block::Numbered(items) => items
                .iter()
                .map(|(n, item)| format!("{n}. {item}"))
                .collect::<Vec<_>>()
                .join("\n"),
            Block::Table(rows) => rows
                .iter()
                .map(|row| format!("| {} |", row.join(" | ")))
                .collect::<Vec<_>>()
                .join("\n"),
            Block::Rule => continue,
        };
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&piece);
    }
    out
}

fn plan_node(src: &str) -> (String, String) {
    let src = src.trim();
    if let Some(open) = src.find('[') {
        let id = src[..open].trim().to_owned();
        let label = src[open + 1..].trim_end_matches(']').trim().to_owned();
        (id, label)
    } else {
        (src.to_owned(), src.to_owned())
    }
}

pub fn parse_plan(src: &str) -> Plan {
    let mut order: Vec<String> = Vec::new();
    let mut labels: HashMap<String, String> = HashMap::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    for line in src.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        let parts: Vec<&str> = line.split("-->").map(str::trim).collect();
        if parts.len() != 2 {
            continue;
        }
        let (a, la) = plan_node(parts[0]);
        let (b, lb) = plan_node(parts[1]);
        for id in [&a, &b] {
            if !labels.contains_key(id) {
                order.push(id.clone());
            }
        }
        if la != a {
            labels.insert(a.clone(), la);
        } else {
            labels.entry(a.clone()).or_insert(la);
        }
        if lb != b {
            labels.insert(b.clone(), lb);
        } else {
            labels.entry(b.clone()).or_insert(lb);
        }
        pairs.push((a, b));
    }
    let index: HashMap<String, usize> = order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.clone(), i))
        .collect();
    let mut edges = Vec::new();
    let mut incoming = vec![0usize; order.len()];
    for (a, b) in pairs {
        if let (Some(&a), Some(&b)) = (index.get(&a), index.get(&b)) {
            edges.push((a, b));
            incoming[b] += 1;
        }
    }
    let mut depth = vec![0usize; order.len()];
    let mut queue: Vec<usize> = (0..order.len()).filter(|i| incoming[*i] == 0).collect();
    let mut seen = vec![false; order.len()];
    while let Some(i) = queue.pop() {
        if seen[i] {
            continue;
        }
        seen[i] = true;
        for (a, b) in &edges {
            if *a == i {
                depth[*b] = depth[*b].max(depth[i] + 1);
                queue.push(*b);
            }
        }
    }
    Plan {
        labels: order
            .into_iter()
            .map(|id| labels.remove(&id).unwrap_or_default())
            .collect(),
        edges,
        depth,
    }
}

pub fn latest_usage(store: &Store, id: &str) -> (u64, f64) {
    let Some(record) = store.load_chat(id) else {
        return (0, 0.0);
    };
    for message in record.messages.iter().rev() {
        let Some(usage) = message.get("usage") else {
            continue;
        };
        let tokens = usage
            .get("totalTokens")
            .and_then(Value::as_u64)
            .or_else(|| usage.get("total_tokens").and_then(Value::as_u64))
            .unwrap_or(0);
        let cost = usage
            .get("cost")
            .and_then(|c| c.get("total"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if tokens > 0 {
            return (tokens, cost);
        }
    }
    (0, 0.0)
}

pub fn load_chat(store: &Store, id: &str) -> Option<Chat> {
    let record = store.load_chat(id)?;
    let mut rows: Vec<Row> = Vec::new();
    let mut plan: Option<Plan> = None;
    for message in &record.messages {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        let content = message.get("content").cloned().unwrap_or(Value::Null);
        if role == "user" {
            let text = match &content {
                Value::String(s) => s.clone(),
                Value::Array(parts) => parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            if !text.trim().is_empty() {
                rows.push(Row::User(text));
            }
            continue;
        }
        if role != "assistant" {
            continue;
        }
        for (kind, part) in content_parts(&content) {
            match kind.as_str() {
                "text" => {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        let blocks = parse_markdown(text);
                        if !blocks.is_empty() {
                            rows.push(Row::Assistant(blocks));
                        }
                    }
                }
                "toolCall" => {
                    let name = part.get("name").and_then(Value::as_str).unwrap_or("");
                    if name == "plan_init"
                        && let Some(mermaid) = part
                            .get("arguments")
                            .and_then(|a| a.get("mermaid"))
                            .and_then(Value::as_str)
                    {
                        plan = Some(parse_plan(mermaid));
                    }
                    if (name == "quiz" || name == "probe")
                        && let Some(quiz) = quiz_from_call(&part)
                    {
                        rows.push(Row::Quiz(quiz));
                    }
                    if name == "problem" {
                        let args = part.get("arguments").cloned().unwrap_or(Value::Null);
                        let statement = args
                            .get("statement")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        if !statement.is_empty() {
                            rows.push(Row::Problem(Problem {
                                statement,
                                mode: args
                                    .get("mode")
                                    .and_then(Value::as_str)
                                    .unwrap_or("recall")
                                    .to_owned(),
                                solution: args
                                    .get("modelSolution")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_owned(),
                                concluded: None,
                            }));
                        }
                    }
                    if name == "conclude"
                        && let Some(Row::Problem(problem)) =
                            rows.iter_mut().rev().find(|r| matches!(r, Row::Problem(_)))
                    {
                        let args = part.get("arguments").cloned().unwrap_or(Value::Null);
                        problem.concluded = Some(Concluded {
                            outcome: args
                                .get("outcome")
                                .and_then(Value::as_str)
                                .unwrap_or("movedOn")
                                .to_owned(),
                            verdict: args
                                .get("verdict")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    // Attach quiz answers from tool results.
    for message in &record.messages {
        if message.get("role").and_then(Value::as_str) != Some("toolResult") {
            continue;
        }
        let name = message
            .get("toolName")
            .and_then(Value::as_str)
            .unwrap_or("");
        if name != "quiz" && name != "probe" {
            continue;
        }
        let call_id = message.get("toolCallId").and_then(Value::as_str);
        let text = match message.get("content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(parts)) => parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => continue,
        };
        if let Some(Row::Quiz(quiz)) = rows.iter_mut().rev().find(|r| {
            matches!(r, Row::Quiz(q) if q.answered.is_none()
                && call_id.is_some()
                && q.call_id.as_deref() == call_id)
        }) {
            apply_result(quiz, &text);
        }
    }
    if rows.len() > 10 {
        rows = rows.split_off(rows.len() - 10);
    }
    Some(Chat {
        id: record.id,
        title: record.title,
        kind: record.kind.unwrap_or_else(|| "teaching".to_owned()),
        rows,
        plan,
    })
}

pub fn load_graph(store: &Store) -> Graph {
    let dir = store.root().join("graph");
    let mut nodes: Vec<Node> = Vec::new();
    let mut by_id: HashMap<String, usize> = HashMap::new();
    let positions: HashMap<String, (f32, f32)> =
        std::fs::read_to_string(dir.join("positions.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.get("positions").cloned())
            .and_then(|v| v.as_object().cloned())
            .map(|map| {
                map.into_iter()
                    .filter_map(|(id, p)| {
                        Some((
                            id,
                            (p.get("x")?.as_f64()? as f32, p.get("y")?.as_f64()? as f32),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
    if let Ok(text) = std::fs::read_to_string(dir.join("nodes.jsonl")) {
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let id = v.get("id").and_then(Value::as_str).unwrap_or_default();
            let (x, y) = positions.get(id).copied().unwrap_or((0.0, 0.0));
            let chat = v
                .get("provenance")
                .and_then(Value::as_array)
                .and_then(|p| p.first())
                .and_then(|p| p.get("chatId"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            by_id.insert(id.to_owned(), nodes.len());
            nodes.push(Node {
                name: v
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                statement: v
                    .get("statement")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                x,
                y,
                chat,
            });
        }
    }
    let mut edges = Vec::new();
    let mut chains = Vec::new();
    if let Ok(text) = std::fs::read_to_string(dir.join("chains.jsonl")) {
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(conclusion) = v
                .get("conclusion")
                .and_then(Value::as_str)
                .and_then(|c| by_id.get(c))
            else {
                continue;
            };
            let mut names = Vec::new();
            if let Some(premises) = v.get("premises").and_then(Value::as_array) {
                for p in premises {
                    if let Some(id) = p.as_str().and_then(|p| by_id.get(p)) {
                        edges.push((*id, *conclusion));
                        names.push(nodes[*id].name.clone());
                    }
                }
            }
            chains.push(Chain {
                premises: names,
                conclusion: nodes[*conclusion].name.clone(),
            });
        }
    }
    let mut sums: HashMap<String, (f32, f32, usize)> = HashMap::new();
    for n in &nodes {
        let e = sums.entry(n.chat.clone()).or_default();
        e.0 += n.x;
        e.1 += n.y;
        e.2 += 1;
    }
    // Normalize positions to 0..1 for layout.
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for n in &nodes {
        min_x = min_x.min(n.x);
        max_x = max_x.max(n.x);
        min_y = min_y.min(n.y);
        max_y = max_y.max(n.y);
    }
    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);
    for n in &mut nodes {
        n.x = (n.x - min_x) / span_x;
        n.y = (n.y - min_y) / span_y;
    }
    let titles: HashMap<String, String> = sessions(store)
        .into_iter()
        .map(|s| (s.id, s.title))
        .collect();
    let clusters = sums
        .into_iter()
        .filter_map(|(chat, (x, y, n))| {
            let title = titles.get(&chat)?.clone();
            let nx = (x / n as f32 - min_x) / span_x;
            let ny = (y / n as f32 - min_y) / span_y;
            Some((title, nx, ny))
        })
        .collect();
    Graph {
        nodes,
        edges,
        clusters,
        chains,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_parses_numbered_lists_and_tables() {
        let blocks = parse_markdown("1. one\n2. two\n\n| A | B |\n|---|---|\n| 1 | 2 |\n");
        assert!(
            matches!(&blocks[0], Block::Numbered(items) if items.len() == 2 && items[1].0 == 2)
        );
        assert!(
            matches!(&blocks[1], Block::Table(rows) if rows.len() == 2 && rows[0] == ["A", "B"])
        );
    }

    #[test]
    fn quiz_results_match_their_own_call() {
        let root = std::env::temp_dir().join(format!("br-rune-test-{}", std::process::id()));
        let chats = root.join("chats");
        std::fs::create_dir_all(&chats).unwrap();
        let record = serde_json::json!({
            "id": "chat_test",
            "title": "Test",
            "createdAt": "2026-01-01T00:00:00Z",
            "updatedAt": "2026-01-01T00:00:00Z",
            "messages": [
                {"role": "assistant", "content": [{"type": "toolCall", "id": "c1", "name": "quiz",
                    "arguments": {"question": "Q1", "options": [{"label": "a"}]}}]},
                {"role": "toolResult", "toolName": "quiz", "toolCallId": "c1",
                    "content": "{\"kind\":\"quiz\",\"dontKnow\":true}"},
                {"role": "assistant", "content": [{"type": "toolCall", "id": "c2", "name": "quiz",
                    "arguments": {"question": "Q2", "options": [{"label": "b"}]}}]}
            ]
        });
        std::fs::write(chats.join("chat_test.json"), record.to_string()).unwrap();
        let chat = load_chat(&Store::new(root), "chat_test").unwrap();
        let quizzes: Vec<&Quiz> = chat
            .rows
            .iter()
            .filter_map(|r| if let Row::Quiz(q) = r { Some(q) } else { None })
            .collect();
        assert_eq!(quizzes.len(), 2);
        assert!(quizzes[0].answered.is_some());
        assert!(quizzes[1].answered.is_none());
    }

    #[test]
    fn plan_keeps_full_labels_and_layers() {
        let wiki = "graph TD\n  u0[Data as numbers] --> u2[Sample mean]\n  u1[Sorting] --> center[Center: mean median]\n  u2 --> center\n";
        let plan = parse_plan(wiki);
        assert_eq!(plan.labels.len(), 4);
        assert!(plan.labels.iter().any(|l| l == "Data as numbers"));
        assert!(plan.labels.iter().any(|l| l == "Center: mean median"));
        assert_eq!(plan.edges.len(), 3);
        let center = plan
            .labels
            .iter()
            .position(|l| l == "Center: mean median")
            .unwrap();
        assert!(plan.depth[center] >= 1);
    }

    #[test]
    fn markdown_splits_blocks_and_round_trips_text() {
        let md = "# Title\n\nA paragraph with $x^2$.\n\n$$\\frac{a}{b}$$\n\n- one\n- two\n";
        let blocks = parse_markdown(md);
        assert!(matches!(blocks[0], Block::Heading(1, _)));
        assert!(matches!(blocks[1], Block::Para(_)));
        assert!(matches!(blocks[2], Block::Display(_)));
        assert!(matches!(blocks[3], Block::Bullet(ref items) if items.len() == 2));
        let text = blocks_text(&blocks);
        assert!(text.contains("Title"));
        assert!(text.contains("\\frac{a}{b}"));
    }
}
