//! Context compaction, ported from LearnHarness.

use serde_json::Value;

pub const KEEP_RECENT_TOKENS: u64 = 20_000;
pub const COMPACTION_RESERVE_TOKENS: u64 = 16_384;
pub const DEFAULT_COMPACTION_THRESHOLD_TOKENS: u64 = 150_000;
pub const COMPACTED_HISTORY_HEADER: &str = "[COMPACTED HISTORY — SUMMARY BELOW]";

fn text_chars(content: &Value) -> u64 {
    match content {
        Value::String(text) => text.chars().count() as u64,
        Value::Array(parts) => parts
            .iter()
            .map(|part| match part.get("type").and_then(Value::as_str) {
                Some("text") | Some("thinking") => part
                    .get("text")
                    .or_else(|| part.get("thinking"))
                    .and_then(Value::as_str)
                    .map(|text| text.chars().count() as u64)
                    .unwrap_or(0),
                Some("toolCall") => {
                    part.get("name")
                        .and_then(Value::as_str)
                        .map(|name| name.chars().count() as u64)
                        .unwrap_or(0)
                        + part
                            .get("arguments")
                            .map(|args| args.to_string().chars().count() as u64)
                            .unwrap_or(0)
                }
                _ => 0,
            })
            .sum(),
        _ => 0,
    }
}

pub fn message_tokens(message: &Value) -> u64 {
    text_chars(message.get("content").unwrap_or(&Value::Null)).div_ceil(4)
}

pub fn usage_tokens(message: &Value) -> Option<u64> {
    let usage = message.get("usage")?;
    let total = usage
        .get("totalTokens")
        .or_else(|| usage.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
        + usage.get("cacheRead").and_then(Value::as_u64).unwrap_or(0)
        + usage.get("cacheWrite").and_then(Value::as_u64).unwrap_or(0);
    (total > 0).then_some(total)
}

/// Usage-anchored estimate: provider usage of the newest assistant message
/// plus chars/4 for everything after it. Compaction markers are skipped.
fn estimate_plain(messages: &[Value]) -> u64 {
    let mut anchor = None;
    for (index, message) in messages.iter().enumerate().rev() {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        if role == "custom" {
            continue;
        }
        if role == "assistant"
            && let Some(tokens) = usage_tokens(message)
        {
            anchor = Some((index, tokens));
            break;
        }
    }
    match anchor {
        Some((index, tokens)) => {
            tokens
                + messages[index + 1..]
                    .iter()
                    .filter(|message| message.get("role").and_then(Value::as_str) != Some("custom"))
                    .map(message_tokens)
                    .sum::<u64>()
        }
        None => messages
            .iter()
            .filter(|message| message.get("role").and_then(Value::as_str) != Some("custom"))
            .map(message_tokens)
            .sum(),
    }
}

pub fn estimate_tokens(messages: &[Value]) -> u64 {
    if let Some(marker) = active_marker(messages)
        && let Some(start) = kept_index(messages, marker)
    {
        return message_tokens(marker) + estimate_plain(&messages[start..]);
    }
    estimate_plain(messages)
}

pub fn active_marker(messages: &[Value]) -> Option<&Value> {
    messages.iter().rev().find(|message| {
        message.get("role").and_then(Value::as_str) == Some("custom")
            && message.get("customType").and_then(Value::as_str) == Some("compaction")
            && message
                .get("details")
                .and_then(|details| details.get("firstKeptMessageId"))
                .and_then(Value::as_str)
                .is_some()
    })
}

pub fn kept_index(messages: &[Value], marker: &Value) -> Option<usize> {
    let id = marker.get("details")?.get("firstKeptMessageId")?.as_str()?;
    messages
        .iter()
        .position(|message| message.get("id").and_then(Value::as_str) == Some(id))
}

pub fn cut_index(messages: &[Value], keep_recent: u64) -> usize {
    let start = active_marker(messages)
        .and_then(|marker| kept_index(messages, marker))
        .unwrap_or(0);
    let points: Vec<usize> = (start..messages.len())
        .filter(|index| {
            matches!(
                messages[*index].get("role").and_then(Value::as_str),
                Some("user" | "assistant" | "custom")
            )
        })
        .collect();
    let Some(&first) = points.first() else {
        return start;
    };
    let mut cut = first;
    let mut accumulated = 0;
    for index in (start..messages.len()).rev() {
        let tokens = message_tokens(&messages[index]);
        if tokens == 0 {
            continue;
        }
        accumulated += tokens;
        if accumulated >= keep_recent {
            cut = points
                .iter()
                .copied()
                .find(|candidate| *candidate >= index)
                .unwrap_or_else(|| *points.last().unwrap());
            break;
        }
    }
    cut
}

pub fn serialize_for_summary(messages: &[Value]) -> String {
    let mut lines = Vec::new();
    for message in messages {
        let role = message.get("role").and_then(Value::as_str).unwrap_or("");
        let content = message.get("content").cloned().unwrap_or(Value::Null);
        match role {
            "user" => {
                if let Some(text) = content.as_str() {
                    lines.push(format!("[LEARNER] {text}"));
                }
            }
            "assistant" => {
                let mut blocks = Vec::new();
                if let Value::Array(parts) = &content {
                    for part in parts {
                        match part.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                if let Some(text) = part.get("text").and_then(Value::as_str) {
                                    blocks.push(text.to_owned());
                                }
                            }
                            Some("toolCall") => {
                                let name = part.get("name").and_then(Value::as_str).unwrap_or("");
                                let args = part.get("arguments").cloned().unwrap_or(Value::Null);
                                blocks.push(format!("(tool call {name}: {args})"));
                            }
                            _ => {}
                        }
                    }
                }
                if !blocks.is_empty() {
                    lines.push(format!("[TUTOR] {}", blocks.join("\n")));
                }
            }
            "toolResult" => {
                let name = message
                    .get("toolName")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let text = match &content {
                    Value::String(text) => text.clone(),
                    Value::Array(parts) => parts
                        .iter()
                        .filter_map(|part| part.get("text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => String::new(),
                };
                let text = if text.chars().count() > 2_000 {
                    format!(
                        "{}…[truncated]",
                        text.chars().take(2_000).collect::<String>()
                    )
                } else {
                    text
                };
                lines.push(format!("[RESULT for {name}] {text}"));
            }
            "custom" => {
                if message.get("customType").and_then(Value::as_str) != Some("plan-block")
                    && let Some(text) = content.as_str()
                {
                    lines.push(format!("[SYSTEM NOTE] {text}"));
                }
            }
            _ => {}
        }
    }
    lines.join("\n\n")
}

pub fn plan_tree_text(tree: &Value) -> Option<String> {
    let nodes = tree.get("nodes")?.as_array()?;
    if nodes.is_empty() {
        return None;
    }
    let mut lines = Vec::new();
    for node in nodes {
        let id = node.get("id").and_then(Value::as_str).unwrap_or("");
        let title = node.get("title").and_then(Value::as_str).unwrap_or("");
        lines.push(format!("- [upcoming] {title} ({id})"));
    }
    if let Some(edges) = tree.get("edges").and_then(Value::as_array) {
        let by_id: std::collections::HashMap<&str, &str> = nodes
            .iter()
            .filter_map(|node| Some((node.get("id")?.as_str()?, node.get("title")?.as_str()?)))
            .collect();
        for edge in edges {
            let from = edge.get("from").and_then(Value::as_str).unwrap_or("");
            let to = edge.get("to").and_then(Value::as_str).unwrap_or("");
            lines.push(format!(
                "- {} → {} ({from} → {to})",
                by_id.get(from).copied().unwrap_or(from),
                by_id.get(to).copied().unwrap_or(to)
            ));
        }
    }
    Some(lines.join("\n"))
}

pub fn build_prompt(span_text: &str, plan: Option<&str>, kind: &str) -> String {
    let plan_section =
        plan.unwrap_or("No plan tree exists in this session (nothing was persisted).");
    format!(
        "You are compacting the earlier part of a {kind} session — writing the summary your future self will read INSTEAD of that transcript. Everything before the cut is replaced by what you write; everything after stays verbatim.\n\nWrite EXACTLY these sections: ## The learner's understanding, ## The teaching arc, ## The plan dependency tree, ## The current moment.\n\nPlan tree:\n{plan_section}\n\nTRANSCRIPT TO SUMMARIZE:\n\n{span_text}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn usage_anchors_the_estimate() {
        let messages = vec![
            json!({"role":"user","content":"x".repeat(4000)}),
            json!({"role":"assistant","usage":{"totalTokens":100},"content":"done"}),
            json!({"role":"user","content":"y".repeat(400)}),
        ];
        assert_eq!(estimate_tokens(&messages), 200);
    }

    #[test]
    fn estimate_uses_summary_plus_retained_tail_after_compaction() {
        let messages = vec![
            json!({"id":"m0","role":"user","content":"x".repeat(40000)}),
            json!({"id":"m1","role":"user","content":"tail"}),
            json!({"id":"m2","role":"assistant","usage":{"totalTokens":100},"content":"ok"}),
            json!({"role":"custom","customType":"compaction","content":"summary","details":{"firstKeptMessageId":"m1"}}),
        ];
        let tokens = estimate_tokens(&messages);
        assert!(tokens < 200, "{tokens}");
    }

    #[test]
    fn cut_keeps_recent_tail_and_projects_after_marker() {
        let messages: Vec<Value> = (0..20)
            .map(|i| json!({"id": format!("m{i}"), "role": "user", "content": "x".repeat(4000)}))
            .collect();
        let cut = cut_index(&messages, 4_000);
        assert!(cut > 0 && cut < messages.len());
        let kept = messages[cut]["id"].as_str().unwrap().to_owned();
        let marker = json!({"role":"custom","customType":"compaction","content":"summary","details":{"firstKeptMessageId":kept}});
        let start = kept_index(&messages, &marker).unwrap();
        assert_eq!(messages[start]["id"], messages[cut]["id"]);
    }
}
