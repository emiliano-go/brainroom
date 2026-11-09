//! LearnHarness's skin: palette, type, icons and the Rune styles the views use.

use rune::{Color, Family, Icon, TextStyle};

fn srgb_decode(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn rgb(hex: u32) -> Color {
    Color {
        r: srgb_decode(((hex >> 16) & 0xff) as f32 / 255.0),
        g: srgb_decode(((hex >> 8) & 0xff) as f32 / 255.0),
        b: srgb_decode((hex & 0xff) as f32 / 255.0),
        a: 1.0,
    }
}

fn rgba(hex: u32, a: f32) -> Color {
    let mut c = rgb(hex);
    c.a = a;
    c
}

pub struct Theme {
    pub dark: bool,
    pub bg: Color,
    pub card: Color,
    pub raised: Color,
    pub border: Color,
    pub border_soft: Color,
    pub fg: Color,
    pub muted: Color,
    pub faint: Color,
    pub hover: Color,
    pub pressed: Color,
    pub gold: Color,
    pub green: Color,
    pub red: Color,
}

pub fn theme(dark: bool) -> Theme {
    if dark {
        Theme {
            dark,
            bg: rgb(0x0b0d0e),
            card: rgb(0x141618),
            raised: rgb(0x1b1e21),
            border: rgb(0x30353a),
            border_soft: rgb(0x22262a),
            fg: rgb(0xe6e6e6),
            muted: rgb(0x8b8f94),
            faint: rgb(0x5b6066),
            hover: rgba(0xffffff, 0.04),
            pressed: rgba(0xffffff, 0.07),
            gold: rgb(0xe6b450),
            green: rgb(0x69c779),
            red: rgb(0xd95b5b),
        }
    } else {
        Theme {
            dark,
            bg: rgb(0xf4f5f7),
            card: rgb(0xffffff),
            raised: rgb(0xeceef1),
            border: rgb(0xd3d6db),
            border_soft: rgb(0xe3e5e9),
            fg: rgb(0x1a1a1a),
            muted: rgb(0x6b6b6b),
            faint: rgb(0x8d8d8d),
            hover: rgba(0x000000, 0.04),
            pressed: rgba(0x000000, 0.07),
            gold: rgb(0xc9962e),
            green: rgb(0x2f8f45),
            red: rgb(0xc23b3b),
        }
    }
}

const MONO: Family = Family::Mono;
const PROSE_FAMILY: Family = Family::Named("Source Sans 3");
const SERIF: Family = Family::Named("CMU Serif");

/// Chrome text: JetBrains Mono.
pub fn mono(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight)
        .family(MONO)
        .lh((size * 1.4).round())
}

/// Teaching prose: Source Sans 3 at the reading size.
pub fn prose() -> TextStyle {
    TextStyle::new(16.0, 400.0).family(PROSE_FAMILY).lh(24.0)
}

/// Titles: CMU Serif bold.
pub fn serif(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight)
        .family(SERIF)
        .lh((size * 1.3).round())
}

pub fn section_title() -> TextStyle {
    serif(19.0, 700.0)
}

pub const ICON_MARK: Icon = Icon {
    name: "lh-mark",
    view: 24.0,
    body: r#"<circle cx="9.5" cy="12" r="6.2" fill="currentColor"/><circle cx="15" cy="12" r="5.6" stroke="currentColor" stroke-width="1.6"/>"#,
};

pub const PANEL_LEFT: Icon = Icon {
    name: "lh-panel-left",
    view: 24.0,
    body: r#"<rect x="3" y="3" width="18" height="18" rx="2" stroke="currentColor" stroke-width="1.6"/><path d="M9 3v18" stroke="currentColor" stroke-width="1.6"/>"#,
};

pub const PLUS: Icon = Icon {
    name: "lh-plus",
    view: 24.0,
    body: r#"<path d="M12 5v14M5 12h14" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>"#,
};

pub const DUMBBELL: Icon = Icon {
    name: "lh-dumbbell",
    view: 24.0,
    body: r#"<path d="M4 9v6M7 7v10M17 7v10M20 9v6M7 12h10" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/>"#,
};

pub const SEARCH: Icon = Icon {
    name: "lh-search",
    view: 24.0,
    body: r#"<circle cx="11" cy="11" r="7" stroke="currentColor" stroke-width="1.6"/><path d="m20 20-3.6-3.6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>"#,
};

pub const SETTINGS: Icon = Icon {
    name: "lh-settings",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="1.6"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4v-.2a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.6-1H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3 1.7 1.7 0 0 0 1-1.6v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round"/>"#,
};

pub const MIC: Icon = Icon {
    name: "lh-mic",
    view: 24.0,
    body: r#"<rect x="9" y="2" width="6" height="12" rx="3" stroke="currentColor" stroke-width="1.6"/><path d="M5 10a7 7 0 0 0 14 0M12 19v3" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>"#,
};

pub const ARROW_UP: Icon = Icon {
    name: "lh-arrow-up",
    view: 24.0,
    body: r#"<path d="M12 19V5M5 12l7-7 7 7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub const CHEVRON_DOWN: Icon = Icon {
    name: "lh-chevron-down",
    view: 24.0,
    body: r#"<path d="m6 9 6 6 6-6" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub const STOP: Icon = Icon {
    name: "lh-stop",
    view: 24.0,
    body: r#"<rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor"/>"#,
};

pub const PLAN: Icon = Icon {
    name: "lh-plan",
    view: 24.0,
    body: r#"<circle cx="6" cy="6" r="2.5" stroke="currentColor" stroke-width="1.5"/><circle cx="6" cy="18" r="2.5" stroke="currentColor" stroke-width="1.5"/><circle cx="18" cy="8" r="2.5" stroke="currentColor" stroke-width="1.5"/><path d="M6 8.5v7M18 10.5a7 7 0 0 1-7 7H8.5" stroke="currentColor" stroke-width="1.5"/>"#,
};

pub const CIRCLE: Icon = Icon {
    name: "lh-circle",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="8.5" stroke="currentColor" stroke-width="1.7"/>"#,
};

pub const UPLOAD: Icon = Icon {
    name: "lh-upload",
    view: 24.0,
    body: r#"<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M17 8l-5-5-5 5M12 3v12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub const CHAIN: Icon = Icon {
    name: "lh-chain",
    view: 24.0,
    body: r#"<path d="M8 6h13M8 12h13M8 18h13" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/><circle cx="3.5" cy="6" r="1.3" fill="currentColor"/><circle cx="3.5" cy="12" r="1.3" fill="currentColor"/><circle cx="3.5" cy="18" r="1.3" fill="currentColor"/>"#,
};

pub const CHECK: Icon = Icon {
    name: "lh-check",
    view: 24.0,
    body: r#"<path d="m5 12 5 5L20 7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub const THINKING: Icon = Icon {
    name: "lh-thinking",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="8.5" stroke="currentColor" stroke-width="1.5"/><path d="M8.5 12h.01M12 12h.01M15.5 12h.01" stroke="currentColor" stroke-width="2.4" stroke-linecap="round"/>"#,
};
