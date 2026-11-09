//! LearnHarness-compatible prompt composition.
//!
//! Source fragments live in `crates/agent/prompts/` (copied from LearnHarness).
//! Overrides mirror that tree under `<data>/prompts/`; the settings switch at
//! `agent.prompts["<path>"]` decides whether an existing override replaces the
//! source. Composition order and the `---` separator match LearnHarness.

use std::path::Path;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentKind {
    Prober,
    Teacher,
    Practice,
}

struct Segment {
    path: &'static str,
    text: &'static str,
}

const BASE: &str = include_str!("../prompts/system-prompt.md");

const TEACHER: &[Segment] = &[
    Segment {
        path: "teacher/05-method.md",
        text: include_str!("../prompts/teacher/05-method.md"),
    },
    Segment {
        path: "teacher/10-planning.md",
        text: include_str!("../prompts/teacher/10-planning.md"),
    },
    Segment {
        path: "teacher/20-teaching.md",
        text: include_str!("../prompts/teacher/20-teaching.md"),
    },
    Segment {
        path: "teacher/28-predictions.md",
        text: include_str!("../prompts/teacher/28-predictions.md"),
    },
    Segment {
        path: "teacher/30-question-principles.md",
        text: include_str!("../prompts/teacher/30-question-principles.md"),
    },
    Segment {
        path: "teacher/40-jobs.md",
        text: include_str!("../prompts/teacher/40-jobs.md"),
    },
    Segment {
        path: "teacher/50-demos.md",
        text: include_str!("../prompts/teacher/50-demos.md"),
    },
];

const PROBER: &[Segment] = &[Segment {
    path: "prober/10-probe-doctrine.md",
    text: include_str!("../prompts/prober/10-probe-doctrine.md"),
}];

const PRACTICE: &[Segment] = &[
    Segment {
        path: "practice/00-base.md",
        text: include_str!("../prompts/practice/00-base.md"),
    },
    Segment {
        path: "practice/10-doctrine.md",
        text: include_str!("../prompts/practice/10-doctrine.md"),
    },
];

const SHARED: &[Segment] = &[
    Segment {
        path: "shared/10-voice.md",
        text: include_str!("../prompts/shared/10-voice.md"),
    },
    Segment {
        path: "shared/15-accuracy.md",
        text: include_str!("../prompts/shared/15-accuracy.md"),
    },
    Segment {
        path: "shared/16-mental-model.md",
        text: include_str!("../prompts/shared/16-mental-model.md"),
    },
    Segment {
        path: "shared/20-question-tools.md",
        text: include_str!("../prompts/shared/20-question-tools.md"),
    },
    Segment {
        path: "shared/25-confidence.md",
        text: include_str!("../prompts/shared/25-confidence.md"),
    },
    Segment {
        path: "shared/30-learner-files.md",
        text: include_str!("../prompts/shared/30-learner-files.md"),
    },
    Segment {
        path: "shared/40-formatting.md",
        text: include_str!("../prompts/shared/40-formatting.md"),
    },
];

const PRACTICE_SHARED: &[&str] = &[
    "shared/10-voice.md",
    "shared/15-accuracy.md",
    "shared/16-mental-model.md",
    "shared/40-formatting.md",
];

fn source_segments(kind: AgentKind) -> Vec<Segment> {
    let mut out = Vec::new();
    let own = match kind {
        AgentKind::Prober => PROBER,
        AgentKind::Teacher => TEACHER,
        AgentKind::Practice => PRACTICE,
    };
    if kind != AgentKind::Practice {
        out.push(Segment {
            path: "system-prompt.md",
            text: BASE,
        });
    }
    out.extend(own.iter().map(|s| Segment {
        path: s.path,
        text: s.text,
    }));
    for segment in SHARED {
        if kind == AgentKind::Practice && !PRACTICE_SHARED.contains(&segment.path) {
            continue;
        }
        out.push(Segment {
            path: segment.path,
            text: segment.text,
        });
    }
    out
}

fn valid_override_path(path: &str) -> bool {
    if path == "system-prompt.md" {
        return true;
    }
    let Some((dir, file)) = path.split_once('/') else {
        return false;
    };
    if file.contains('/') || file.contains("..") || file.is_empty() {
        return false;
    }
    if !matches!(dir, "shared" | "prober" | "teacher" | "practice") {
        return false;
    }
    let base = file.strip_suffix(".md").unwrap_or("");
    let (prefix, name) = base.split_once('-').unwrap_or(("", ""));
    !prefix.is_empty()
        && prefix.chars().all(|c| c.is_ascii_digit())
        && !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn override_enabled(settings: &serde_json::Value, path: &str) -> bool {
    settings
        .pointer(&format!("/agent/prompts/{}", path.replace('/', "~1")))
        .map(|value| match value {
            serde_json::Value::Bool(enabled) => *enabled,
            serde_json::Value::Object(map) => map
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            _ => false,
        })
        .unwrap_or(false)
}

fn override_text(data_dir: &Path, path: &str) -> Option<String> {
    if !valid_override_path(path) {
        return None;
    }
    std::fs::read_to_string(data_dir.join("prompts").join(path)).ok()
}

fn teaching_style(settings: &serde_json::Value) -> &'static str {
    match settings
        .pointer("/agent/teachingStyle")
        .and_then(|v| v.as_str())
        .unwrap_or("guided")
    {
        "socratic" => {
            "Teaching style: SOCRATIC. Prefer drawing the step out of him with questions before revealing it — but instruction must still follow an attempt, and a check must never be the only teaching."
        }
        _ => {
            "Teaching style: GUIDED. Teach first: explain the step fully, work an example end-to-end, then check. Use a guided check before an independent check, and fade support across exposures."
        }
    }
}

pub fn compose(kind: AgentKind, data_dir: &Path) -> String {
    let settings = std::fs::read_to_string(data_dir.join("settings.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or(serde_json::Value::Null);
    let mut parts: Vec<String> = source_segments(kind)
        .into_iter()
        .map(|segment| {
            let text = if override_enabled(&settings, segment.path) {
                override_text(data_dir, segment.path).unwrap_or_else(|| segment.text.to_owned())
            } else {
                segment.text.to_owned()
            };
            text.trim().to_owned()
        })
        .filter(|text| !text.is_empty())
        .collect();
    if kind == AgentKind::Teacher {
        parts.push(teaching_style(&settings).to_owned());
    }
    parts.join("\n\n---\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_data(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("brainroom-prompts-{}-{label}", std::process::id()))
    }

    #[test]
    fn teacher_prompt_carries_method_and_math_contract() {
        let prompt = compose(AgentKind::Teacher, &temp_data("teacher"));
        assert!(prompt.contains("The method — two principles"));
        assert!(prompt.contains("Use `$...$` for inline math"));
        assert!(prompt.contains("Teaching style: GUIDED"));
    }

    #[test]
    fn practice_prompt_excludes_teacher_method_and_tool_doctrine() {
        let prompt = compose(AgentKind::Practice, &temp_data("practice"));
        assert!(prompt.contains("The practice session"));
        assert!(prompt.contains("Use `$...$` for inline math"));
        assert!(!prompt.contains("Unconditional truths first"));
        assert!(!prompt.contains("`probe_done`"));
    }

    #[test]
    fn enabled_override_replaces_its_segment() {
        let dir = temp_data("override");
        let prompts = dir.join("prompts");
        std::fs::create_dir_all(&prompts).unwrap();
        std::fs::write(prompts.join("system-prompt.md"), "OVERRIDDEN BASE").unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"agent":{"prompts":{"system-prompt.md":true}}}"#,
        )
        .unwrap();
        let prompt = compose(AgentKind::Teacher, &dir);
        assert!(prompt.starts_with("OVERRIDDEN BASE"));
        std::fs::write(
            dir.join("settings.json"),
            r#"{"agent":{"prompts":{"system-prompt.md":false}}}"#,
        )
        .unwrap();
        assert!(!compose(AgentKind::Teacher, &dir).starts_with("OVERRIDDEN BASE"));
    }
}
