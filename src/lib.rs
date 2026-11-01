//! Synchronous accent generation and fixed/image-derived toolkit application. Toolkit installers own finite file and desktop-preference updates.
mod config_io;
pub mod gtk;
pub mod matugen;
pub mod qt;
pub mod theme;
mod vscode;
pub use config_io::Paths;
use palette::convert::FromColorUnclamped;
use palette::{FromColor, Oklch, Srgb};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Dark,
    Light,
}
impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

pub const CANONICAL: &str = "oklch(0.397103719 0.120772653 24.4920635)";
const L0: f64 = 0.397103719;
const C0: f64 = 0.120772653;
const H0: f64 = 24.4920635;
pub const DECORATIVE: [&str; 14] = ["reference", "accent_bg", "accent_fg", "accent_text", "accent_hover", "accent_pressed", "accent_soft", "selection_bg", "selection_fg", "selection_inactive_bg", "selection_inactive_fg", "focus", "link", "link_visited"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seed {
    pub l: f64,
    pub c: f64,
    pub h: f64,
}

impl FromStr for Seed {
    type Err = String;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let lower = input.trim().to_ascii_lowercase();
        let body = lower.strip_prefix("oklch(").and_then(|s| s.strip_suffix(')')).ok_or("expected oklch(L C H), with no alpha")?;
        let tokens: Vec<_> = body.split_whitespace().collect();
        if tokens.len() != 3 || body.contains(['/', ',']) {
            return Err("expected three OKLCH components; alpha is not supported".into());
        }
        let number = |s: &str| s.parse::<f64>().map_err(|_| format!("invalid number: {s}"));
        let l = if let Some(s) = tokens[0].strip_suffix('%') { number(s)? / 100.0 } else { number(tokens[0])? };
        let c = number(tokens[1])?;
        let h = number(tokens[2].strip_suffix("deg").unwrap_or(tokens[2]))?;
        Self::new(l, c, h)
    }
}
impl Seed {
    pub fn new(l: f64, c: f64, h: f64) -> Result<Self, String> {
        if ![l, c, h].iter().all(|x| x.is_finite()) || !(0.0..=1.0).contains(&l) || c < 0.0 {
            return Err("OKLCH requires finite numbers, lightness in 0..1 and non-negative chroma".into());
        }
        let h = h.rem_euclid(360.0) % 360.0;
        Ok(Self { l: if l == 0.0 { 0.0 } else { l }, c: if c == 0.0 { 0.0 } else { c }, h: if h == 0.0 { 0.0 } else { h } })
    }
    fn at_lightness(self, l: f64) -> Self {
        gamut_map(Self { l, ..self })
    }
}
impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Round-trip precision for the reference; this is also the typed profile format.
        let h = if self.h >= 359.99999995 { 0.0 } else { self.h };
        write!(f, "oklch({:.9} {:.9} {:.7})", self.l, self.c, h)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TypedColor {
    pub color: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModePalette {
    pub palette: BTreeMap<String, TypedColor>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Modes<T> {
    pub dark: T,
    pub light: T,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub version: u32,
    pub seed: String,
    pub named_accent: String,
    pub palettes: Modes<ModePalette>,
    pub hex: Modes<BTreeMap<String, String>>,
}

pub fn profiles() -> Modes<ModePalette> {
    Modes { dark: serde_json::from_str(include_str!("../assets/accents-dark.json")).unwrap(), light: serde_json::from_str(include_str!("../assets/accents-light.json")).unwrap() }
}
fn neutrals() -> Modes<ModePalette> {
    Modes { dark: serde_json::from_str(include_str!("../assets/neutrals-dark.json")).unwrap(), light: serde_json::from_str(include_str!("../assets/neutrals-light.json")).unwrap() }
}

/// Toolkit export truncates RGB channels to u8. The approved palettes use this calibration; Color::to_rgba8() rounds and would change their colours.
pub fn rgb(css: &str) -> Result<[u8; 3], String> {
    let col = css.parse::<csscolorparser::Color>().map_err(|e| e.to_string())?;
    Ok([(col.r * 255.0) as u8, (col.g * 255.0) as u8, (col.b * 255.0) as u8])
}
pub fn hex(css: &str) -> Result<String, String> {
    let [r, g, b] = rgb(css)?;
    Ok(format!("#{r:02X}{g:02X}{b:02X}"))
}
fn hex_map(mode: &ModePalette) -> Result<BTreeMap<String, String>, String> {
    mode.palette.iter().map(|(k, v)| Ok((k.clone(), hex(&v.color)?))).collect()
}
fn in_gamut(color: Seed) -> bool {
    // Check the coefficients that will actually be serialized. This matters near black/white, where rounding L can move a chroma-boundary candidate out of gamut even though its unrounded coefficients were valid.
    let l = (color.l * 1e9).round() / 1e9;
    let c = (color.c * 1e9).round() / 1e9;
    let h = (color.h * 1e7).round() / 1e7;
    let srgb: Srgb<f64> = Srgb::from_color_unclamped(Oklch::new(l, c, h));
    [srgb.red, srgb.green, srgb.blue].iter().all(|v| v.is_finite() && *v >= -1e-7 && *v <= 1.0 + 1e-7)
}
fn gamut_map(mut color: Seed) -> Seed {
    if in_gamut(color) {
        return color;
    }
    // At the endpoints hue has no RGB meaning. Keep its metadata, remove chroma.
    if color.l <= 0.0 || color.l >= 1.0 {
        color.c = 0.0;
        return color;
    }
    let mut low = 0.0;
    // No sRGB colour has OKLCH chroma >= 1. This also bounds huge finite inputs.
    let mut high = color.c.min(1.0);
    for _ in 0..40 {
        let mid = (low + high) / 2.0;
        if in_gamut(Seed { c: mid, ..color }) {
            low = mid;
        } else {
            high = mid;
        }
    }
    // Leave room for nine-decimal serialization at the gamut boundary.
    color.c = (low - 1e-9).max(0.0);
    color
}
fn transformed(base: Seed, seed: Seed) -> Seed {
    // The curated coefficients were calibrated for truncated 8-bit export. Some channel-255 entries overshoot mathematical sRGB by a few millionths. Preserve this validated calibration exactly at the reference.
    if (seed.l - L0).abs() < 1e-12 && (seed.c - C0).abs() < 1e-12 && (seed.h - H0).abs() < 1e-12 {
        return base;
    }
    let l = if base.l <= L0 { seed.l * base.l / L0 } else { seed.l + (1.0 - seed.l) * (base.l - L0) / (1.0 - L0) };
    let c = (base.c * (seed.c.min(1e100) / C0)).min(1.0);
    gamut_map(Seed { l, c, h: (base.h + seed.h - H0).rem_euclid(360.0) })
}
fn luminance(rgb: [f64; 3]) -> f64 {
    let linear = rgb.map(|x| if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) });
    0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
}
/// Worst contrast at opacity 1 and 0.8, composited over black and white.
pub fn contrast(fg: [u8; 3], bg: [u8; 3]) -> f64 {
    [(1.0, 0.0), (0.8, 0.0), (0.8, 1.0)]
        .into_iter()
        .map(|(a, wall)| {
            let composite = |color: [u8; 3]| color.map(|x| f64::from(x) / 255.0 * a + wall * (1.0 - a));
            let f = luminance(composite(fg));
            let b = luminance(composite(bg));
            (f.max(b) + 0.05) / (f.min(b) + 0.05)
        })
        .fold(f64::INFINITY, f64::min)
}
fn bytes(color: Seed) -> [u8; 3] {
    rgb(&color.to_string()).expect("generated OKLCH is valid CSS")
}
fn passes(fg: Seed, backgrounds: &[[u8; 3]], minimum: f64) -> bool {
    backgrounds.iter().all(|bg| contrast(bytes(fg), *bg) >= minimum)
}

/// Search the entire lightness range, then refine the nearest valid interval. All predicates use serialized, quantized RGB, including gamut mapping. Work is bounded even for quantization discontinuities and extreme seeds.
fn nearest(color: Seed, valid: impl Fn(Seed) -> bool) -> Option<Seed> {
    if valid(color) {
        return Some(color);
    }
    let mut best: Option<Seed> = None;
    for i in 0..=512 {
        let candidate = color.at_lightness(f64::from(i) / 512.0);
        if best.is_some_and(|b| (b.l - color.l).abs() <= (candidate.l - color.l).abs()) {
            continue;
        }
        if valid(candidate) {
            best = Some(candidate);
        }
    }
    let mut good = best?;
    let mut bad_l = color.l;
    for _ in 0..30 {
        let mid = color.at_lightness((good.l + bad_l) / 2.0);
        if valid(mid) {
            good = mid;
        } else {
            bad_l = mid.l;
        }
    }
    Some(good)
}
fn repair_group(fg: Seed, fills: &[Seed]) -> Result<(Seed, Vec<Seed>), String> {
    let backgrounds: Vec<_> = fills.iter().copied().map(bytes).collect();
    if let Some(foreground) = nearest(fg, |x| passes(x, &backgrounds, 4.5)) {
        return Ok((foreground, fills.to_vec()));
    }
    // No common foreground fits these fills. Only now move fills, considering both polarities; bright input is allowed to use a dark foreground.
    [0.0, 1.0]
        .into_iter()
        .filter_map(|l| {
            let foreground = fg.at_lightness(l);
            let adjusted: Option<Vec<_>> = fills.iter().map(|fill| nearest(*fill, |x| contrast(bytes(foreground), bytes(x)) >= 4.5)).collect();
            adjusted.map(|values| {
                let cost = (foreground.l - fg.l).abs() + values.iter().zip(fills).map(|(a, b)| (a.l - b.l).abs()).sum::<f64>();
                (cost, foreground, values)
            })
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, f, b)| (f, b))
        .ok_or_else(|| "could not resolve shared foreground contrast".into())
}
fn mode_palette(profile: &ModePalette, neutral: &ModePalette, seed: Seed, mode: Mode) -> Result<ModePalette, String> {
    let mut colors: BTreeMap<String, Seed> = DECORATIVE
        .iter()
        .map(|name| {
            let base: Seed = profile.palette[*name].color.parse().expect("embedded profile is valid");
            ((*name).to_string(), transformed(base, seed))
        })
        .collect();
    let fill_names = ["accent_bg", "accent_hover", "accent_pressed"];
    let fills: Vec<_> = fill_names.iter().map(|k| colors[*k]).collect();
    let (fg, fills) = repair_group(colors["accent_fg"], &fills)?;
    colors.insert("accent_fg".into(), fg);
    for (name, value) in fill_names.into_iter().zip(fills) {
        colors.insert(name.into(), value);
    }
    colors.insert("selection_bg".into(), colors["accent_bg"]);
    colors.insert("selection_fg".into(), colors["accent_fg"]);
    let (fg, bg) = repair_group(colors["selection_inactive_fg"], &[colors["selection_inactive_bg"]])?;
    colors.insert("selection_inactive_fg".into(), fg);
    colors.insert("selection_inactive_bg".into(), bg[0]);
    let qt: qt::Profile = serde_json::from_str(match mode {
        Mode::Dark => include_str!("../assets/qt-dark.json"),
        Mode::Light => include_str!("../assets/qt-light.json"),
    })
    .expect("embedded Qt reference is valid");
    let qt_backgrounds: Vec<_> = ["Window", "Base", "AlternateBase", "Button", "ToolTipBase"]
        .into_iter()
        .map(|name| match &qt.qt["active"][name] {
            qt::Colour::Fixed { color } => rgb(color).unwrap(),
            _ => unreachable!("reference Qt surfaces must be fixed"),
        })
        .collect();
    for (role, background_names, minimum) in [("accent_text", &["view_bg", "window_bg", "button_bg", "menu_bg", "popover_bg", "surface_highest"][..], 4.5), ("link", &["view_bg", "window_bg", "button_bg", "menu_bg", "popover_bg", "surface_highest"][..], 4.5), ("link_visited", &["view_bg", "window_bg", "button_bg", "menu_bg", "popover_bg", "surface_highest"][..], 4.5), ("focus", &["window_bg", "view_bg", "button_bg", "button_hover", "button_pressed", "surface_highest"][..], 3.0)] {
        let backgrounds: Vec<_> = background_names.iter().map(|k| rgb(&neutral.palette[*k].color).unwrap()).collect();
        let value = nearest(colors[role], |x| {
            passes(x, &backgrounds, minimum)
                && qt_backgrounds.iter().all(|bg| {
                    if mode == Mode::Light {
                        contrast(bytes(x), *bg) >= minimum
                    } else {
                        // Preserve the approved dark Qt exception for translucent links on SweetAmbar's brighter button/tooltip surfaces.
                        let fg = luminance(bytes(x).map(|v| f64::from(v) / 255.0));
                        let bg = luminance(bg.map(|v| f64::from(v) / 255.0));
                        (fg.max(bg) + 0.05) / (fg.min(bg) + 0.05) >= minimum
                    }
                })
        })
        .ok_or_else(|| format!("could not resolve {role} contrast"))?;
        colors.insert(role.into(), value);
    }
    let mut result = profile.clone();
    for (name, color) in colors {
        result.palette.insert(name, TypedColor { color: color.to_string() });
    }
    Ok(result)
}

/// Generate both modes from immutable profiles. Semantic roles are copied byte for byte. An achromatic seed makes all decorative roles achromatic; hue is retained in metadata so a caller can add chroma later.
pub fn generate(seed: Seed) -> Result<Bundle, String> {
    // Public struct fields are convenient for consumers; still validate here.
    let seed = Seed::new(seed.l, seed.c, seed.h)?;
    let base = profiles();
    let neutrals = neutrals();
    let palettes = Modes { dark: mode_palette(&base.dark, &neutrals.dark, seed, Mode::Dark)?, light: mode_palette(&base.light, &neutrals.light, seed, Mode::Light)? };
    let hex = Modes { dark: hex_map(&palettes.dark)?, light: hex_map(&palettes.light)? };
    Ok(Bundle { version: 1, seed: seed.to_string(), named_accent: named_accent(seed).into(), palettes, hex })
}

/// Closest libadwaita named hue in the Oklab a/b plane. Ignore lightness so a dark red stays red rather than matching a darker purple fallback. Exact toolkit colours still come from the palette; this preference is a fallback.
pub(crate) fn named_accent(seed: Seed) -> &'static str {
    if seed.c < 0.025 {
        return "slate";
    }
    let angle = seed.h.to_radians();
    let reference = [angle.cos(), angle.sin()];
    [("blue", "#3584e4"), ("teal", "#2190a4"), ("green", "#3a944a"), ("yellow", "#c88800"), ("orange", "#ed5b00"), ("red", "#e62d42"), ("pink", "#d56199"), ("purple", "#9141ac"), ("slate", "#6f8396")]
        .into_iter()
        .map(|(name, css)| {
            let [r, g, b] = rgb(css).unwrap().map(|x| f64::from(x) / 255.0);
            let lab: palette::Oklab<f64> = palette::Oklab::from_color(Srgb::new(r, g, b));
            let chroma = lab.a.hypot(lab.b);
            let distance = (reference[0] - lab.a / chroma).powi(2) + (reference[1] - lab.b / chroma).powi(2);
            (name, distance)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
        .0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crimson_is_exact() {
        let bundle = generate(CANONICAL.parse().unwrap()).unwrap();
        assert_eq!(bundle.palettes, profiles());
        assert_eq!(bundle.hex.dark["reference"], "#7B2424");
        assert_eq!(bundle.named_accent, "red");
        assert_eq!(generate("oklch(39.7103719% 0.120772653 384.4920635deg)".parse().unwrap()).unwrap().palettes, profiles());
    }
    #[test]
    fn parsing() {
        assert_eq!("oklch(39.7103719% 0.120772653 384.4920635deg)".parse::<Seed>().unwrap().to_string(), CANONICAL);
        for input in ["red", "#7b2424", "oklch(1.1 0 0)", "oklch(-1 0 0)", "oklch(0.5 -1 0)", "oklch(NaN 0 0)", "oklch(0.5 inf 0)", "oklch(0.5 0 NaN)", "oklch(0.5 0 0 / 1)", "oklch(50%, 0, 0)"] {
            assert!(input.parse::<Seed>().is_err(), "{input}");
        }
        assert_eq!("oklch(0.5 0 -90deg)".parse::<Seed>().unwrap().h, 270.0);
        assert_eq!("oklch(-0 -0 -360deg)".parse::<Seed>().unwrap().to_string(), "oklch(0.000000000 0.000000000 0.0000000)");
        assert_eq!("oklch(0.5 0.1 359.999999999)".parse::<Seed>().unwrap().to_string(), "oklch(0.500000000 0.100000000 0.0000000)");
    }
    #[test]
    fn varied_seeds_keep_semantics_and_contrast() {
        for input in ["oklch(0.55 0.2 260)", "oklch(0.6 0.2 145)", "oklch(0.75 0.17 80)", "oklch(0.5 0 0)", "oklch(0 0 0)", "oklch(1 0 0)", "oklch(0.5 2 315)", "oklch(0 1 0)", "oklch(1 1 120)", "oklch(0.000000001 1e308 359.999999999)", "oklch(0.999999999 0.000000001 -360)"] {
            let bundle = generate(input.parse().unwrap()).unwrap();
            assert_eq!(bundle, generate(input.parse().unwrap()).unwrap());
            for (actual, base, neutral) in [(&bundle.palettes.dark, profiles().dark, neutrals().dark), (&bundle.palettes.light, profiles().light, neutrals().light)] {
                assert_eq!(actual.palette.len(), 29);
                for (name, color) in &actual.palette {
                    if !DECORATIVE.contains(&name.as_str()) {
                        assert_eq!(color, &base.palette[name]);
                    } else {
                        assert!(in_gamut(color.color.parse().unwrap()), "{input}: {name}: {}", color.color);
                    }
                }
                let color = |name: &str| rgb(&actual.palette[name].color).unwrap();
                for bg in ["accent_bg", "accent_hover", "accent_pressed"] {
                    assert!(contrast(color("accent_fg"), color(bg)) >= 4.5);
                }
                assert!(contrast(color("selection_inactive_fg"), color("selection_inactive_bg")) >= 4.5);
                for name in ["accent_text", "link", "link_visited"] {
                    for bg in ["view_bg", "window_bg", "popover_bg", "surface_highest"] {
                        assert!(contrast(color(name), rgb(&neutral.palette[bg].color).unwrap()) >= 4.5);
                    }
                }
            }
        }
    }
}
