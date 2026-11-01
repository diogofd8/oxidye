//! Fixed Breeze palette composition. This path never invokes a palette generator or reads another theme engine's configuration/state.
use crate::{contrast, rgb, Bundle, Mode, Seed, DECORATIVE};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(crate) mod install;
pub const ROLES: [&str; 22] = ["WindowText", "Button", "Light", "Midlight", "Dark", "Mid", "Text", "BrightText", "ButtonText", "Base", "Window", "Shadow", "Highlight", "HighlightedText", "Link", "LinkVisited", "AlternateBase", "NoRole", "ToolTipBase", "ToolTipText", "PlaceholderText", "Accent"];
pub const GROUPS: [&str; 3] = ["active", "inactive", "disabled"];
pub const INACTIVE_SELECTION_GROUP: &str = "Colors:Selection][Inactive";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Colour {
    Fixed { color: String },
    Accent { accent: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Setting {
    Colour(Colour),
    Literal(String),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u32,
    pub name: String,
    pub qt: BTreeMap<String, BTreeMap<String, Colour>>,
    pub kde: BTreeMap<String, BTreeMap<String, Setting>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccentFile {
    pub version: u32,
    pub seed: String,
    pub modes: crate::Modes<BTreeMap<String, String>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rendered {
    pub version: u32,
    pub mode: Mode,
    pub seed: String,
    pub palette: BTreeMap<String, BTreeMap<String, String>>,
    pub kde: BTreeMap<String, BTreeMap<String, String>>,
    pub contrast_checks: Vec<ContrastCheck>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContrastCheck {
    pub pair: String,
    pub target: f64,
    pub opaque: f64,
    pub composited_min: f64,
}

fn opaque(css: &str) -> Result<[u8; 3], String> {
    if !css.starts_with("oklch(") {
        return Err("authored colours must use OKLCH".into());
    }
    let color: csscolorparser::Color = css.parse().map_err(|e| format!("invalid colour {css}: {e}"))?;
    if ![color.r, color.g, color.b, color.a].iter().all(|c| c.is_finite()) || color.a != 1.0 {
        return Err(format!("expected a finite opaque colour: {css}"));
    }
    rgb(css)
}
fn hex(color: [u8; 3]) -> String {
    let [r, g, b] = color;
    format!("#FF{r:02X}{g:02X}{b:02X}")
}
fn triplet(color: [u8; 3]) -> String {
    let [r, g, b] = color;
    format!("{r},{g},{b}")
}
fn bytes(argb: &str) -> [u8; 3] {
    [3, 5, 7].map(|start| u8::from_str_radix(&argb[start..start + 2], 16).unwrap())
}

/// Convert generated colours into the fixed workflow's selected accent source. Semantic colours belong to the fixed profile, not this file.
pub fn accents_from_bundle(bundle: &Bundle) -> AccentFile {
    let extract = |mode: &crate::ModePalette| DECORATIVE.into_iter().map(|name| (name.to_string(), mode.palette[name].color.clone())).collect();
    AccentFile { version: 1, seed: bundle.seed.clone(), modes: crate::Modes { dark: extract(&bundle.palettes.dark), light: extract(&bundle.palettes.light) } }
}
impl AccentFile {
    pub fn colours(&self, mode: Mode) -> &BTreeMap<String, String> {
        match mode {
            Mode::Dark => &self.modes.dark,
            Mode::Light => &self.modes.light,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("unsupported accent file version".into());
        }
        self.seed.parse::<Seed>()?;
        let expected: BTreeSet<_> = DECORATIVE.into_iter().map(str::to_string).collect();
        for (mode, colors) in [("dark", &self.modes.dark), ("light", &self.modes.light)] {
            if colors.keys().cloned().collect::<BTreeSet<_>>() != expected {
                return Err(format!("{mode}: expected exactly the 14 decorative accent roles"));
            }
            for (role, css) in colors {
                css.parse::<Seed>().map_err(|e| format!("{mode}/{role}: {e}"))?;
                opaque(css).map_err(|e| format!("{mode}/{role}: {e}"))?;
            }
        }
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        let data = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let result: Self = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
        result.validate()?;
        Ok(result)
    }
}
impl Profile {
    pub fn mode(&self) -> Result<Mode, String> {
        match self.name.as_str() {
            "OxidyeDark" => Ok(Mode::Dark),
            "OxidyeLight" => Ok(Mode::Light),
            _ => Err("expected a OxidyeDark or OxidyeLight profile".into()),
        }
    }
    pub fn read(path: &Path) -> Result<Self, String> {
        serde_json::from_slice(&fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?).map_err(|e| format!("{}: {e}", path.display()))
    }
    fn validate(&self) -> Result<(), String> {
        self.mode()?;
        if self.version != 1 {
            return Err("expected a version 1 Qt profile".into());
        }
        let expected: BTreeSet<_> = ROLES.into_iter().map(str::to_string).collect();
        if self.qt.keys().map(String::as_str).collect::<BTreeSet<_>>() != GROUPS.into_iter().collect() {
            return Err("profile needs active, inactive and disabled groups".into());
        }
        for (group, colors) in &self.qt {
            if colors.keys().cloned().collect::<BTreeSet<_>>() != expected {
                return Err(format!("{group}: expected all 22 Qt roles"));
            }
        }
        let mut expected_groups: BTreeSet<_> = ["ColorEffects:Disabled", "ColorEffects:Inactive", "Colors:Button", "Colors:Complementary", "Colors:Header", "Colors:Selection", "Colors:Tooltip", "Colors:View", "Colors:Window", "General"].into_iter().collect();
        if self.mode()? == Mode::Light {
            expected_groups.insert(INACTIVE_SELECTION_GROUP);
        }
        if self.kde.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected_groups {
            return Err("unexpected or missing KDE colour groups".into());
        }
        if !matches!(self.kde["General"].get("ColorScheme"), Some(Setting::Literal(name)) if name == &self.name) {
            return Err("General/ColorScheme must match the profile name".into());
        }
        for (group, settings) in &self.kde {
            for (key, value) in settings {
                if key.is_empty() || key.contains(['\n', '\r', '\0', '=', '[', ']']) {
                    return Err("invalid KDE key".into());
                }
                if group == "General" && !["ColorScheme", "AccentColor"].contains(&key.as_str()) {
                    return Err("unexpected General setting".into());
                }
                if let Setting::Literal(value) = value {
                    if value.contains(['\n', '\r', '\0']) {
                        return Err("invalid literal setting".into());
                    }
                    if group.starts_with("Colors:") {
                        return Err("KDE colour entries must be typed colours".into());
                    }
                }
            }
        }
        Ok(())
    }
    pub fn compose(&self, accent: &AccentFile) -> Result<Rendered, String> {
        self.validate()?;
        accent.validate()?;
        let mode = self.mode()?;
        let colours = accent.colours(mode);
        let resolve = |color: &Colour| -> Result<[u8; 3], String> {
            match color {
                Colour::Fixed { color } => opaque(color),
                Colour::Accent { accent: role } => opaque(colours.get(role).ok_or_else(|| format!("unknown accent role {role}"))?),
            }
        };
        let palette = self
            .qt
            .iter()
            .map(|(group, colors)| {
                let colors = colors.iter().map(|(role, color)| Ok((role.clone(), hex(resolve(color)?)))).collect::<Result<_, String>>()?;
                Ok((group.clone(), colors))
            })
            .collect::<Result<_, String>>()?;
        let kde = self
            .kde
            .iter()
            .map(|(group, settings)| {
                let settings = settings
                    .iter()
                    .map(|(key, value)| {
                        let value = match value {
                            Setting::Colour(color) => triplet(resolve(color)?),
                            Setting::Literal(value) => value.clone(),
                        };
                        Ok((key.clone(), value))
                    })
                    .collect::<Result<_, String>>()?;
                Ok((group.clone(), settings))
            })
            .collect::<Result<_, String>>()?;
        let mut output = Rendered { version: 1, mode, seed: accent.seed.clone(), palette, kde, contrast_checks: Vec::new() };
        output.contrast_checks = output.validate(accent)?;
        Ok(output)
    }
}
impl Rendered {
    fn validate(&self, accents: &AccentFile) -> Result<Vec<ContrastCheck>, String> {
        let active = &self.palette["active"];
        let accent = accents.colours(self.mode);
        let mut checks = Vec::new();
        for (fg, bg) in [("WindowText", "Window"), ("Text", "Base"), ("Text", "AlternateBase"), ("ButtonText", "Button"), ("ToolTipText", "ToolTipBase")] {
            checks.push((format!("{fg}/{bg}"), bytes(&active[fg]), bytes(&active[bg]), 4.5, false));
        }
        for group in ["active", "inactive"] {
            let colors = &self.palette[group];
            checks.push((format!("{group} selection"), bytes(&colors["HighlightedText"]), bytes(&colors["Highlight"]), 4.5, true));
        }
        for role in ["link", "link_visited", "accent_text", "focus"] {
            for bg in ["Window", "Base", "AlternateBase", "Button", "ToolTipBase"] {
                checks.push((format!("{role}/{bg}"), opaque(&accent[role])?, bytes(&active[bg]), if role == "focus" { 3.0 } else { 4.5 }, false));
            }
        }
        for role in ["accent_bg", "accent_hover", "accent_pressed"] {
            checks.push((format!("selection foreground/{role}"), opaque(&accent["selection_fg"])?, opaque(&accent[role])?, 4.5, true));
        }
        let mut report = Vec::new();
        for (pair, fg, bg, target, require_composited) in checks {
            let fg_l = crate::luminance(fg.map(|c| f64::from(c) / 255.0));
            let bg_l = crate::luminance(bg.map(|c| f64::from(c) / 255.0));
            let opaque = (fg_l.max(bg_l) + 0.05) / (fg_l.min(bg_l) + 0.05);
            let composited_min = contrast(fg, bg);
            if opaque + 1e-6 < target || ((require_composited || self.mode == Mode::Light) && composited_min + 1e-6 < target) {
                return Err(format!("{pair}: contrast {opaque:.2} opaque / {composited_min:.2} composited; requires {target}"));
            }
            // Report all translucent contrasts without silently altering the approved dark SweetAmbar surfaces or reference accent tones.
            report.push(ContrastCheck { pair, target, opaque, composited_min });
        }
        Ok(report)
    }
    pub fn palette_ini(&self) -> String {
        let mut result = if self.kde.get("General").and_then(|g| g.get("ColorScheme")).is_some_and(|name| name.starts_with("OxidyeMatugen")) {
            "# Image-derived Matugen colours mapped by Oxidye.\n"
        } else {
            match self.mode {
                Mode::Dark => "# Fixed SweetAmbar neutrals and separately selected accents.\n",
                Mode::Light => "# Fixed light neutrals and separately selected accents.\n",
            }
        }
        .to_string();
        result.push_str("# Shared Qt 5/6 palette: Qt 5 ignores the final Accent role.\n[ColorScheme]\n");
        for group in GROUPS {
            let values: Vec<_> = ROLES.iter().map(|name| self.palette[group][*name].as_str()).collect();
            result.push_str(&format!("{group}_colors={}\n", values.join(",")));
        }
        result
    }
    pub fn scheme_ini(&self) -> String {
        edit_ini("", &self.kde)
    }
    pub fn json(&self) -> Result<Vec<u8>, String> {
        json_bytes(self)
    }
}
pub fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>, String> {
    let mut data = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    data.push(b'\n');
    Ok(data)
}

/// Update owned keys while preserving all unowned settings and comments. Values are constructed internally (single-line INI literals), never shell commands.
pub fn edit_ini(input: &str, updates: &BTreeMap<String, BTreeMap<String, String>>) -> String {
    fn flush(result: &mut String, group: &str, updates: &BTreeMap<String, BTreeMap<String, String>>, written: &mut BTreeSet<(String, String)>) {
        if let Some(settings) = updates.get(group) {
            for (key, value) in settings {
                if written.insert((group.into(), key.clone())) {
                    result.push_str(&format!("{key}={value}\n"));
                }
            }
        }
    }
    let mut result = String::new();
    let mut group = String::new();
    let mut seen = BTreeSet::new();
    let mut written = BTreeSet::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            flush(&mut result, &group, updates, &mut written);
            group = trimmed[1..trimmed.len() - 1].into();
            seen.insert(group.clone());
        } else if !trimmed.starts_with(['#', ';']) {
            if let Some((key, _)) = line.split_once('=') {
                let key = key.trim();
                if let Some(value) = updates.get(&group).and_then(|values| values.get(key)) {
                    if written.insert((group.clone(), key.into())) {
                        result.push_str(&format!("{key}={value}\n"));
                    }
                    continue;
                }
            }
        }
        result.push_str(line);
        result.push('\n');
    }
    flush(&mut result, &group, updates, &mut written);
    for (group, settings) in updates {
        if seen.contains(group) {
            continue;
        }
        if !result.is_empty() && !result.ends_with("\n\n") {
            result.push('\n');
        }
        result.push_str(&format!("[{group}]\n"));
        for (key, value) in settings {
            result.push_str(&format!("{key}={value}\n"));
        }
    }
    result
}
