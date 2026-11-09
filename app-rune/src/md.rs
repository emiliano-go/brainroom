//! Markdown blocks for the session transcript, with KaTeX inline and display math.

use rune::{Color, El, InlineIcon, Span, col, el, rich, row, text};
use rune_kit::{Markdown, MarkdownStyle};

use crate::data::Block;
use crate::math;
use crate::theme::{Theme, mono, prose, serif};

fn inline_code_bg(th: &Theme) -> Color {
    if th.dark {
        Color { a: 0.08, ..th.fg }
    } else {
        Color { a: 0.06, ..th.fg }
    }
}

enum Tok {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    Math(String),
}

fn tokenize(src: &str) -> Vec<Tok> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut plain = String::new();
    let mut i = 0;
    let push_plain = |plain: &mut String, out: &mut Vec<Tok>| {
        if !plain.is_empty() {
            out.push(Tok::Text(std::mem::take(plain)));
        }
    };
    while i < bytes.len() {
        if bytes[i] == b'`'
            && let Some(end) = src[i + 1..].find('`')
        {
            push_plain(&mut plain, &mut out);
            out.push(Tok::Code(src[i + 1..i + 1 + end].to_owned()));
            i += end + 2;
            continue;
        }
        if src[i..].starts_with("\\(")
            && let Some(end) = src[i + 2..].find("\\)")
        {
            push_plain(&mut plain, &mut out);
            out.push(Tok::Math(src[i + 2..i + 2 + end].to_owned()));
            i += end + 4;
            continue;
        }
        if bytes[i] == b'$'
            && !src[i..].starts_with("$$")
            && let Some(end) = src[i + 1..].find('$')
        {
            let tex = &src[i + 1..i + 1 + end];
            if !tex.trim().is_empty() && !tex.contains('\n') {
                push_plain(&mut plain, &mut out);
                out.push(Tok::Math(tex.to_owned()));
                i += end + 2;
                continue;
            }
        }
        if src[i..].starts_with("**")
            && let Some(end) = src[i + 2..].find("**")
        {
            push_plain(&mut plain, &mut out);
            out.push(Tok::Bold(src[i + 2..i + 2 + end].to_owned()));
            i += end + 4;
            continue;
        }
        if bytes[i] == b'*'
            && !src[i..].starts_with("**")
            && let Some(end) = src[i + 1..].find('*')
        {
            push_plain(&mut plain, &mut out);
            out.push(Tok::Italic(src[i + 1..i + 1 + end].to_owned()));
            i += end + 2;
            continue;
        }
        let ch = src[i..].chars().next().unwrap();
        plain.push(ch);
        i += ch.len_utf8();
    }
    push_plain(&mut plain, &mut out);
    out
}

/// A paragraph with inline emphasis, code and math.
pub fn para(src: &str, th: &Theme) -> El {
    let mut content = String::new();
    let mut spans: Vec<Span> = Vec::new();
    let mut icons: Vec<(usize, InlineIcon)> = Vec::new();
    for tok in tokenize(src) {
        match tok {
            Tok::Text(s) => content.push_str(&s),
            Tok::Bold(s) => {
                let start = content.len();
                content.push_str(&s);
                spans.push(Span::new(start, content.len()).weight(650.0));
            }
            Tok::Italic(s) => {
                let start = content.len();
                content.push_str(&s);
                spans.push(Span::new(start, content.len()).italic());
            }
            Tok::Code(s) => {
                let start = content.len();
                content.push_str(&s);
                spans.push(
                    Span::new(start, content.len())
                        .mono()
                        .bg(inline_code_bg(th), 3.0),
                );
            }
            Tok::Math(tex) => {
                let at = content.len();
                content.push('\u{200b}');
                if let Some(image) = math::render(&tex, false, 32.0, th.fg) {
                    let pixels = math::square(&image);
                    let side = image.width.max(image.height) as f32 / 2.0;
                    icons.push((at, InlineIcon::image(pixels).size(side).gap(2.0)));
                }
            }
        }
    }
    if content.is_empty() {
        return el().h(0.0);
    }
    let mut e = rich(content, spans).font(prose()).color(th.fg).w_full();
    for (at, icon) in icons {
        e = e.inline_icon(at, icon);
    }
    e
}

/// A display formula, centered at its natural size.
pub fn display(tex: &str, th: &Theme) -> El {
    match math::render(tex, true, 42.0, th.fg) {
        Some(image) => {
            let w = image.width as f32 / 2.0;
            let h = image.height as f32 / 2.0;
            row()
                .w_full()
                .center_y()
                .py(10.0)
                .child(el().size(w, h).pixels(&image.pixels))
        }
        None => text(tex.to_owned()).font(mono(13.0, 400.0)).color(th.red),
    }
}

fn code_block(src: &str, th: &Theme) -> El {
    let style = MarkdownStyle::neutral(th.dark)
        .text(th.fg)
        .muted(th.muted)
        .code_block(th.card, th.border_soft)
        .code_block_font(mono(13.0, 400.0));
    Markdown::new(format!("```text\n{src}\n```\n")).render(&style)
}

/// The whole block list, stacked.
pub fn blocks(blocks: &[Block], th: &Theme) -> El {
    let mut out = col().w_full().gap(14.0);
    for block in blocks {
        match block {
            Block::Para(s) => out = out.child(para(s, th)),
            Block::Heading(level, s) => {
                let size = match level {
                    1 => 25.0,
                    2 => 20.0,
                    3 => 17.0,
                    _ => 15.0,
                };
                out = out.child(
                    text(s.clone())
                        .font(serif(size, 700.0))
                        .color(th.fg)
                        .w_full(),
                )
            }
            Block::Display(tex) => out = out.child(display(tex, th)),
            Block::Code(src) => out = out.child(code_block(src, th)),
            Block::Bullet(items) => {
                let mut list = col().w_full().gap(6.0);
                for item in items {
                    list = list.child(
                        row()
                            .w_full()
                            .gap(10.0)
                            .child(text("•".to_owned()).font(prose()).color(th.muted))
                            .child(para(item, th).grow(1.0)),
                    );
                }
                out = out.child(list);
            }
            Block::Quote(s) => {
                out = out.child(
                    row()
                        .w_full()
                        .gap(12.0)
                        .child(el().w(2.0).fill(th.border))
                        .child(para(s, th).grow(1.0)),
                )
            }
            Block::Rule => out = out.child(el().w_full().h(1.0).fill(th.border_soft)),
        }
    }
    out
}
