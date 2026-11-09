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
