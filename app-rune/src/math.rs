//! KaTeX display and inline math, rasterized with katex-native and cached.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use katex_native::Color as KColor;
use katex_native::katex::KatexContext;
use katex_native::raster::rasterize;
use rune::{Color, Pixels};

type Entry = (Pixels, u32, u32, Vec<u8>);
type Key = (String, bool, u32, u32);
static CACHE: OnceLock<Mutex<HashMap<Key, Entry>>> = OnceLock::new();

/// Rasterized formula: pixels, width and height in device pixels.
#[derive(Clone)]
pub struct MathImage {
    pub pixels: Pixels,
    pub width: u32,
    pub height: u32,
    rgba: Vec<u8>,
}

fn srgb_encode(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

fn key_color(c: Color) -> u32 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (q(c.r) << 24) | (q(c.g) << 16) | (q(c.b) << 8) | q(c.a)
}

/// Render `tex` at `em_px` pixels per em in `color`, cached by its inputs.
pub fn render(tex: &str, display: bool, em_px: f32, color: Color) -> Option<MathImage> {
    let em = (em_px.max(8.0) * 4.0).round() as u32;
    let ck = key_color(color);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some((pixels, width, height, rgba)) = cache
        .lock()
        .ok()?
        .get(&(tex.to_owned(), display, em, ck))
        .cloned()
    {
        return Some(MathImage {
            pixels,
            width,
            height,
            rgba,
        });
    }
    let ctx = KatexContext::default();
    let layout = katex_native::layout(&ctx, tex, display).ok()?;
    let kc = KColor {
        r: srgb_encode(color.r),
        g: srgb_encode(color.g),
        b: srgb_encode(color.b),
        a: (color.a.clamp(0.0, 1.0) * 255.0).round() as u8,
    };
    let raster = rasterize(&layout, em as f64 / 4.0, kc)?;
    let pixels = Pixels::new();
    pixels.set(raster.width, raster.height, raster.rgba.clone());
    let out = MathImage {
        pixels,
        width: raster.width,
        height: raster.height,
        rgba: raster.rgba,
    };
    if let Ok(mut cache) = cache.lock() {
        cache.insert(
            (tex.to_owned(), display, em, ck),
            (out.pixels.clone(), out.width, out.height, out.rgba.clone()),
        );
    }
    Some(out)
}

/// Pad an image to a transparent square so Rune's inline icon slot keeps its aspect.
pub fn square(image: &MathImage) -> Pixels {
    let side = image.width.max(image.height);
    if side == image.width && side == image.height {
        return image.pixels.clone();
    }
    let ox = (side - image.width) / 2;
    let oy = (side - image.height) / 2;
    let row = side as usize * 4;
    let mut buf = vec![0u8; row * side as usize];
    for y in 0..image.height as usize {
        let dst = (y + oy as usize) * row + ox as usize * 4;
        let s = y * image.width as usize * 4;
        buf[dst..dst + image.width as usize * 4]
            .copy_from_slice(&image.rgba[s..s + image.width as usize * 4]);
    }
    let pixels = Pixels::new();
    pixels.set(side, side, buf);
    pixels
}
