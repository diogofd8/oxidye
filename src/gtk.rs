//! Fixed GTK layouts with independently imported fixed and decorative colours.
use crate::{
    contrast,
    qt::{json_bytes, AccentFile},
    rgb, Mode, ModePalette, Seed, TypedColor,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(crate) mod install;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub mode: Mode,
    pub neutrals: BTreeMap<String, TypedColor>,
    pub semantics: BTreeMap<String, TypedColor>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rgba {
    pub rgb: [u8; 3],
    pub alpha: f64,
}
impl Rgba {
    fn parse(css: &str, allow_alpha: bool) -> Result<Self, String> {
        let body = css.trim().strip_prefix("oklch(").and_then(|s| s.strip_suffix(')')).ok_or("GTK source colours must use OKLCH")?;
        let parts: Vec<_> = body.split('/').collect();
        if parts.len() > 2 {
            return Err("invalid colour alpha".into());
        }
        let coordinates: Vec<_> = parts[0].split_whitespace().collect();
        if coordinates.len() != 3 {
            return Err("expected three OKLCH coordinates".into());
        }
        let number = |s: &str| s.parse::<f64>().map_err(|_| "invalid colour coordinate".to_string());
        let l = number(coordinates[0])?;
        let c = number(coordinates[1])?;
        let h = number(coordinates[2])?;
        // The fixed white calibration sits slightly above 1 to survive RGB truncation.
        if ![l, c, h].iter().all(|v| v.is_finite()) || !(0.0..=1.001).contains(&l) || c < 0.0 {
            return Err("invalid fixed OKLCH colour".into());
        }
        let alpha = if parts.len() == 2 { number(parts[1].trim())? } else { 1.0 };
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) || (!allow_alpha && alpha != 1.0) {
            return Err("unexpected or invalid colour alpha".into());
        }
        let color: csscolorparser::Color = css.parse().map_err(|e| format!("invalid colour: {e}"))?;
        if ![color.r, color.g, color.b, color.a].iter().all(|v| v.is_finite()) {
            return Err("non-finite colour".into());
        }
        Ok(Self { rgb: rgb(css)?, alpha })
    }
    fn css(&self) -> String {
        let [r, g, b] = self.rgb;
        if self.alpha == 1.0 {
            format!("rgb({r}, {g}, {b})")
        } else {
            format!("rgba({r}, {g}, {b}, {})", self.alpha)
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rendered {
    pub version: u32,
    pub mode: Mode,
    pub seed: String,
    pub named_accent: String,
    pub colours: BTreeMap<String, Rgba>,
    pub contrast_checks: Vec<crate::qt::ContrastCheck>,
}
impl Profile {
    pub fn read(path: &Path) -> Result<Self, String> {
        serde_json::from_slice(&fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| e.to_string())
    }
    pub fn compose(&self, accents: &AccentFile) -> Result<Rendered, String> {
        if self.version != 1 {
            return Err("unsupported GTK profile version".into());
        }
        let neutral_reference: ModePalette = serde_json::from_str(include_str!("../assets/neutrals-dark.json")).map_err(|e| e.to_string())?;
        if self.neutrals.keys().ne(neutral_reference.palette.keys()) {
            return Err("unexpected or missing GTK neutral roles".into());
        }
        let expected: BTreeSet<_> = ["error", "destructive", "success", "warning", "info"].iter().flat_map(|name| ["bg", "fg", "text"].map(|suffix| format!("{name}_{suffix}"))).collect();
        if self.semantics.keys().cloned().collect::<BTreeSet<_>>() != expected {
            return Err("expected all 15 fixed semantic roles".into());
        }
        accents.validate()?;
        let mut colours = BTreeMap::new();
        for (role, value) in self.neutrals.iter().chain(&self.semantics) {
            let allow_alpha = ["shade", "shade_strong", "shadow", "scrollbar_outline"].contains(&role.as_str());
            colours.insert(role.clone(), Rgba::parse(&value.color, allow_alpha).map_err(|e| format!("{role}: {e}"))?);
        }
        for (role, value) in accents.colours(self.mode) {
            colours.insert(role.clone(), Rgba::parse(value, false)?);
        }
        let mut output = Rendered { version: 1, mode: self.mode, seed: accents.seed.clone(), named_accent: crate::named_accent(accents.seed.parse::<Seed>()?).into(), colours, contrast_checks: Vec::new() };
        output.validate()?;
        Ok(output)
    }
}
impl Rendered {
    pub(crate) fn validate(&mut self) -> Result<(), String> {
        let reference: ModePalette = serde_json::from_str(include_str!("../assets/neutrals-dark.json")).map_err(|e| e.to_string())?;
        let expected: BTreeSet<_> = reference.palette.keys().cloned().chain(crate::DECORATIVE.map(str::to_string)).chain(["error", "destructive", "success", "warning", "info"].into_iter().flat_map(|name| ["bg", "fg", "text"].map(move |suffix| format!("{name}_{suffix}")))).collect();
        if self.version != 1 || self.colours.keys().cloned().collect::<BTreeSet<_>>() != expected {
            return Err("invalid GTK rendered palette version or colour roles".into());
        }
        self.seed.parse::<Seed>()?;
        if !["blue", "teal", "green", "yellow", "orange", "red", "pink", "purple", "slate"].contains(&self.named_accent.as_str()) || self.colours.iter().any(|(role, color)| !color.alpha.is_finite() || !(0.0..=1.0).contains(&color.alpha) || (color.alpha != 1.0 && !["shade", "shade_strong", "shadow", "scrollbar_outline"].contains(&role.as_str()))) {
            return Err("invalid GTK rendered accent or opacity".into());
        }
        self.contrast_checks.clear();
        let mut pairs = Vec::new();
        for (fg, bg) in [("window_fg", "window_bg"), ("view_fg", "view_bg"), ("view_fg", "view_alt"), ("button_fg", "button_bg"), ("button_fg", "button_hover"), ("button_fg", "button_pressed"), ("toggle_fg", "toggle_bg"), ("headerbar_fg", "headerbar_bg"), ("sidebar_fg", "sidebar_bg"), ("card_fg", "card_bg"), ("popover_fg", "popover_bg"), ("popover_fg", "menu_bg"), ("popover_fg", "menu_hover"), ("dialog_fg", "dialog_bg"), ("tooltip_fg", "tooltip_bg"), ("osd_fg", "osd_bg"), ("selection_fg", "selection_bg"), ("selection_inactive_fg", "selection_inactive_bg")] {
            pairs.push((fg.to_string(), bg.to_string(), 4.5));
        }
        for fg in ["link", "link_visited", "accent_text", "muted_fg"] {
            for bg in ["window_bg", "view_bg", "button_bg", "menu_bg"] {
                pairs.push((fg.into(), bg.into(), 4.5));
            }
        }
        for bg in ["accent_bg", "accent_hover", "accent_pressed"] {
            pairs.push(("accent_fg".into(), bg.into(), 4.5));
        }
        for bg in ["window_bg", "view_bg", "button_bg"] {
            pairs.push(("focus".into(), bg.into(), 3.0));
        }
        for semantic in ["error", "destructive", "success", "warning", "info"] {
            pairs.push((format!("{semantic}_fg"), format!("{semantic}_bg"), 4.5));
            for bg in ["window_bg", "view_bg", "button_bg"] {
                pairs.push((format!("{semantic}_text"), bg.into(), 4.5));
            }
        }
        for (fg, bg, target) in pairs {
            let foreground = self.colours[&fg].rgb;
            let background = self.colours[&bg].rgb;
            let a = crate::luminance(foreground.map(|v| f64::from(v) / 255.0));
            let b = crate::luminance(background.map(|v| f64::from(v) / 255.0));
            let opaque = (a.max(b) + 0.05) / (a.min(b) + 0.05);
            let composited_min = contrast(foreground, background);
            if composited_min + 1e-6 < target {
                return Err(format!("{fg}/{bg}: contrast {opaque:.2} opaque / {composited_min:.2} composited; requires {target}"));
            }
            self.contrast_checks.push(crate::qt::ContrastCheck { pair: format!("{fg}/{bg}"), target, opaque, composited_min });
        }
        Ok(())
    }
    fn colour_css(&self, decorative: bool) -> String {
        let mut css = String::from("/* OKLCH source exported as RGB for GTK. */\n");
        for (role, colour) in &self.colours {
            if crate::DECORATIVE.contains(&role.as_str()) == decorative {
                css.push_str(&format!("@define-color dt_{role} {};\n", colour.css()));
            }
        }
        css
    }
    pub fn files(&self, layouts: &[String; 2]) -> Result<BTreeMap<String, String>, String> {
        for layout in layouts {
            if !layout.contains("@import url(\"fixed-colors.css\");") || !layout.contains("@import url(\"accent-colors.css\");") || layout.contains("{{") {
                return Err("GTK layouts must import the two neighbouring colour files".into());
            }
            for suffix in layout.split("@dt_").skip(1) {
                let name: String = suffix.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
                if !self.colours.contains_key(&name) {
                    return Err(format!("unknown GTK layout colour: {name}"));
                }
            }
        }
        Ok(BTreeMap::from([("fixed-colors.css".into(), self.colour_css(false)), ("accent-colors.css".into(), self.colour_css(true)), ("gtk3.css".into(), layouts[0].clone()), ("gtk4.css".into(), layouts[1].clone()), ("palette.json".into(), String::from_utf8(json_bytes(self)?).unwrap())]))
    }
}
