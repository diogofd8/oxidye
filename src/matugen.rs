//! Optional image generation through Matugen. Its cache is private to the subprocess; retained files are complete toolkit outputs.
mod colours;

use crate::{
    gtk,
    theme::{self, Config, PreparedMode},
    Mode, Modes, Paths, TypedColor,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

pub(crate) const MANIFEST: &str = "oxidye/matugen/selection.json";
// Bump when matugen.toml, source selection or other generation parameters change. Matugen's key does not include custom colours.
pub const GENERATION_PROFILE_VERSION: u32 = 1;
const RENDER_VERSION: u32 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Scheme {
    SchemeContent,
    SchemeExpressive,
    SchemeFidelity,
    SchemeFruitSalad,
    SchemeMonochrome,
    SchemeNeutral,
    SchemeRainbow,
    #[default]
    SchemeTonalSpot,
    SchemeVibrant,
    SchemeSmart,
}
impl Scheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SchemeContent => "scheme-content",
            Self::SchemeExpressive => "scheme-expressive",
            Self::SchemeFidelity => "scheme-fidelity",
            Self::SchemeFruitSalad => "scheme-fruit-salad",
            Self::SchemeMonochrome => "scheme-monochrome",
            Self::SchemeNeutral => "scheme-neutral",
            Self::SchemeRainbow => "scheme-rainbow",
            Self::SchemeTonalSpot => "scheme-tonal-spot",
            Self::SchemeVibrant => "scheme-vibrant",
            Self::SchemeSmart => "scheme-smart",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    version: u32,
    pub image: String,
    pub scheme: Scheme,
    seed: String,
}
#[derive(Deserialize)]
struct Export {
    #[serde(deserialize_with = "read_colours")]
    colors: BTreeMap<String, Modes<TypedColor>>,
}
fn read_colours<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<BTreeMap<String, Modes<TypedColor>>, D::Error> {
    #[derive(Deserialize)]
    struct MaterialModes {
        dark: TypedColor,
        light: TypedColor,
    }
    let colours = BTreeMap::<String, MaterialModes>::deserialize(deserializer)?;
    Ok(colours.into_iter().map(|(name, colour)| (name, Modes { dark: colour.dark, light: colour.light })).collect())
}
pub(crate) fn locations(mode: Mode) -> (String, String, String) {
    let title = match mode {
        Mode::Dark => "Dark",
        Mode::Light => "Light",
    };
    let mode = mode.as_str();
    (format!("oxidye/matugen/qt/{mode}.conf"), format!("oxidye/matugen/qt/OxidyeMatugen{title}.colors"), format!("oxidye/matugen/gtk/{mode}"))
}
fn cache_root() -> Result<PathBuf, String> {
    let root = env::var_os("XDG_CACHE_HOME").filter(|s| !s.is_empty()).map(PathBuf::from).or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".cache"))).ok_or("set XDG_CACHE_HOME or HOME")?;
    if !root.is_absolute() {
        return Err("XDG_CACHE_HOME must be an absolute directory".into());
    }
    Ok(root.join(format!("oxidye/matugen/generation-v{GENERATION_PROFILE_VERSION}")))
}
fn generate(paths: &Paths, image: &Path, scheme: Scheme, mode: Mode) -> Result<(Export, String), String> {
    let image = fs::canonicalize(image).map_err(|e| format!("cannot read image {}: {e}", image.display()))?;
    if !image.is_file() {
        return Err("--image must name a readable image file".into());
    }
    let image = image.to_str().filter(|s| !s.chars().any(char::is_control)).ok_or("image path must be UTF-8 without control characters")?.to_string();
    let cache = cache_root()?;
    let mut config = tempfile::NamedTempFile::new_in(&paths.state).map_err(|e| e.to_string())?;
    config.write_all(include_bytes!("../assets/matugen.toml")).map_err(|e| e.to_string())?;
    let result = Command::new("matugen").args(["image", &image, "--mode", mode.as_str(), "--type", scheme.as_str(), "--json", "hex", "--dry-run", "--source-color-index", "0", "--quiet", "--config"]).arg(config.path()).env("XDG_CACHE_HOME", cache).env("NO_COLOR", "1").output().map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { "Matugen is not installed or is missing from PATH; install matugen to generate an image theme. Fixed themes and rendered image themes remain usable.".into() } else { format!("cannot run Matugen: {e}") })?;
    if !result.status.success() {
        return Err(format!("Matugen generation failed ({}): {}", result.status, String::from_utf8_lossy(&result.stderr).trim()));
    }
    let export = serde_json::from_slice(&result.stdout).map_err(|e| format!("invalid Matugen JSON output (expected the Matugen 4.2 colour format): {e}"))?;
    Ok((export, image))
}
fn files(gtk: &gtk::Rendered, layouts: &[String; 2]) -> Result<BTreeMap<String, String>, String> {
    let mut files = gtk.files(layouts)?;
    for name in ["fixed-colors.css", "accent-colors.css"] {
        let text = files.get_mut(name).unwrap();
        *text = text.replacen("/* OKLCH source exported as RGB for GTK. */", "/* Matugen colours and OKLCH-derived tones exported as RGB for GTK. */", 1);
    }
    Ok(files)
}
fn load(paths: &Paths) -> Result<(Modes<PreparedMode>, Manifest), String> {
    let load = || -> Result<_, String> {
        let manifest: Manifest = serde_json::from_slice(&fs::read(paths.config.join(MANIFEST)).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        if manifest.version != RENDER_VERSION || !Path::new(&manifest.image).is_absolute() || manifest.image.chars().any(char::is_control) {
            return Err("invalid rendered image-theme metadata".into());
        }
        let mode = |mode| -> Result<PreparedMode, String> {
            let (palette, scheme, directory) = locations(mode);
            let root = paths.config.join(directory);
            let mut gtk: gtk::Rendered = serde_json::from_slice(&fs::read(root.join("palette.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            if gtk.mode != mode || gtk.seed != manifest.seed {
                return Err("rendered image-theme appearance or seed mismatch".into());
            }
            gtk.validate()?;
            colours::validate_boundaries(&gtk)?;
            let layouts = [fs::read_to_string(root.join("gtk3.css")).map_err(|e| e.to_string())?, fs::read_to_string(root.join("gtk4.css")).map_err(|e| e.to_string())?];
            let files = files(&gtk, &layouts)?;
            let qt = colours::qt(&gtk)?;
            for (name, expected) in &files {
                if fs::read_to_string(root.join(name)).map_err(|e| e.to_string())? != *expected {
                    return Err(format!("rendered GTK file {name} is inconsistent"));
                }
            }
            for (name, expected) in [(palette, qt.palette_ini()), (scheme, qt.scheme_ini())] {
                if fs::read_to_string(paths.config.join(name)).map_err(|e| e.to_string())? != expected {
                    return Err("rendered Qt files are inconsistent".into());
                }
            }
            Ok(PreparedMode { qt, gtk, files })
        };
        Ok((Modes { dark: mode(Mode::Dark)?, light: mode(Mode::Light)? }, manifest))
    };
    load().map_err(|e| format!("no complete, valid rendered Matugen theme: {e}; supply --image to generate both appearances"))
}
pub(crate) fn prepare(paths: &Paths, config: &Config, image: Option<&Path>, scheme: Scheme, mode: Mode) -> Result<(Modes<PreparedMode>, Manifest), String> {
    let Some(image) = image else {
        return load(paths);
    };
    let (export, image) = generate(paths, image, scheme, mode)?;
    let layouts = theme::layouts(paths, config)?;
    let compose = |mode| -> Result<PreparedMode, String> {
        let gtk = colours::gtk(&export.colors, mode)?;
        let qt = colours::qt(&gtk)?;
        let files = files(&gtk, &layouts)?;
        Ok(PreparedMode { qt, gtk, files })
    };
    let prepared = Modes { dark: compose(Mode::Dark)?, light: compose(Mode::Light)? };
    if prepared.dark.gtk.seed != prepared.light.gtk.seed {
        return Err("Matugen returned different image seeds for light and dark".into());
    }
    let manifest = Manifest { version: RENDER_VERSION, image, scheme, seed: prepared.dark.gtk.seed.clone() };
    Ok((prepared, manifest))
}
