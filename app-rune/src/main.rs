//! Brainroom's native Rune shell.

mod ai;
mod data;
mod math;
mod md;
mod theme;
mod views;

use rune::{App, AppSpec};

const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/JetBrainsMono.ttf"),
    include_bytes!("../assets/SourceSans3.ttf"),
    include_bytes!("../assets/cmunbx.ttf"),
];

fn scene_session(s: &mut views::State) {
    s.open_first();
}

fn scene_settings(s: &mut views::State) {
    s.open_first();
    s.settings_open = true;
}

fn scene_quiz(s: &mut views::State) {
    s.demo_quiz();
}

fn scene_problem(s: &mut views::State) {
    s.demo_problem();
}

fn scene_math(s: &mut views::State) {
    s.view = views::View::Session;
    s.chat = Some(data::Chat {
        id: "demo".to_owned(),
        title: "Math".to_owned(),
        kind: "teaching".to_owned(),
        rows: vec![data::Row::Assistant(vec![
            data::Block::Para(
                "For $f(n) = 100n + 5$, as $n \\to \\infty$ the term $100n$ dominates.".to_owned(),
            ),
            data::Block::Numbered(vec![
                (1, "First step with $x^2$.".to_owned()),
                (2, "Second **bold** step.".to_owned()),
            ]),
            data::Block::Table(vec![
                vec!["Notation".to_owned(), "Meaning".to_owned()],
                vec!["$O(n)$".to_owned(), "at most".to_owned()],
                vec!["$\\Theta(n)$".to_owned(), "exactly".to_owned()],
            ]),
            data::Block::Display("\\Theta(n^2)".to_owned()),
        ])],
        plan: None,
    });
}

fn scene_streaming(s: &mut views::State) {
    s.open_first();
    s.busy = true;
    s.stream_text = "Here is the idea: **sorting** turns a list into order statistics, and every center or spread measure rests on those.".to_owned();
}

fn poll(s: &mut views::State, _t: f64, _tx: &mut rune::Tx) -> bool {
    s.poll()
}

fn main() {
    let spec = AppSpec {
        scenes: &[
            ("session", scene_session),
            ("settings", scene_settings),
            ("streaming", scene_streaming),
            ("math", scene_math),
            ("quiz", scene_quiz),
            ("problem", scene_problem),
        ],
        poll: Some(poll),
        fonts: FONTS,
        families: ("Source Sans 3", "JetBrains Mono"),
        min_size: Some((1360.0, 820.0)),
        ..AppSpec::new("Brainroom", (1440.0, 900.0), views::State::new, views::view)
    };
    App::new(spec).run();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    #[test]
    fn home_and_session_build() {
        let env = Env {
            system_dark: true,
            scale: 2.0,
        };
        let mut rt = Runtime::new(views::State::new(), views::view, (1440.0, 900.0), env);
        rt.frame(0.0);
        assert!(rt.find("sidebar").is_some(), "the sidebar mounts");
        scene_session(&mut rt.state);
        rt.frame(1.0 / 60.0);
        assert!(
            rt.state.chat.as_ref().is_some_and(|c| !c.rows.is_empty()),
            "the session loads"
        );
    }
}
