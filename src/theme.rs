//! On-demand selection of the approved fixed Qt/GTK themes and generated accents. Renderers and toolkit settings plans are shared with the individual commands.
use crate::{
    config_io::{self, atomic, read, Paths, Snapshot},
    generate, gtk,
    qt::{self, accents_from_bundle, edit_ini, json_bytes, AccentFile},
    Mode, Modes, Seed,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

const CONFIG: &str = "oxidye/config.json";
const APPLIED: &str = "oxidye/applied.json";
const PREFERENCES: [&str; 4] = ["gtk-theme", "color-scheme", "accent-color", "icon-theme"];
const SETTINGS: [&str; 9] = ["kdeglobals", "qt5ct/qt5ct.conf", "qt6ct/qt6ct.conf", "gtk-3.0/gtk.css", "gtk-4.0/gtk.css", "gtk-3.0/settings.ini", "gtk-4.0/settings.ini", "xsettingsd/xsettingsd.conf", "oxidye/qt/applied.json"];
const GTK_FILES: [&str; 5] = ["fixed-colors.css", "accent-colors.css", "gtk3.css", "gtk4.css", "palette.json"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QtFiles {
    pub profiles: Modes<String>,
    pub palettes: Modes<String>,
    pub schemes: Modes<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GtkLayouts {
    pub gtk3: String,
    pub gtk4: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GtkFiles {
    pub profiles: Modes<String>,
    pub layouts: GtkLayouts,
    pub outputs: Modes<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub mode: Mode,
    pub icons: Modes<String>,
    pub accents: String,
    pub qt: QtFiles,
    pub gtk: GtkFiles,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vscode: Option<Modes<String>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Applied {
    pub version: u32,
    pub mode: Mode,
    pub seed: String,
    pub named_accent: String,
    pub icon_theme: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vscode_theme: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Backup {
    version: u32,
    config_root: PathBuf,
    files: Vec<Snapshot>,
    preferences: Option<BTreeMap<String, String>>,
}
fn pick(modes: &Modes<String>, mode: Mode) -> &String {
    match mode {
        Mode::Dark => &modes.dark,
        Mode::Light => &modes.light,
    }
}
fn safe_relative(value: &str) -> bool {
    !value.is_empty() && !value.contains(['\n', '\r', '\0']) && value.split('/').all(|p| !p.is_empty() && p != "." && p != "..") && Path::new(value).components().all(|p| matches!(p, Component::Normal(_)))
}
fn relative(value: &str) -> Result<String, String> {
    if !safe_relative(value) {
        return Err(format!("expected a relative path beneath oxidye: {value:?}"));
    }
    Ok(format!("oxidye/{value}"))
}
pub fn config_path(paths: &Paths) -> PathBuf {
    paths.config.join(CONFIG)
}
pub fn backup_path(paths: &Paths) -> PathBuf {
    paths.state.join("theme-backup.json")
}
impl Config {
    pub fn read(paths: &Paths) -> Result<Self, String> {
        let config: Self = serde_json::from_str(&read(&config_path(paths))?.ok_or("oxidye/config.json is missing; install the supplied picker configuration")?).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }
    fn output_paths(&self) -> Result<Vec<String>, String> {
        let mut files = vec![relative(&self.accents)?, CONFIG.into(), APPLIED.into(), "oxidye/gtk/applied.json".into()];
        files.extend(SETTINGS.map(str::to_string));
        if self.vscode.is_some() {
            files.push(crate::vscode::SETTINGS.into());
        }
        for mode in [Mode::Dark, Mode::Light] {
            files.push(relative(pick(&self.qt.palettes, mode))?);
            files.push(relative(pick(&self.qt.schemes, mode))?);
            let dir = relative(pick(&self.gtk.outputs, mode))?;
            files.extend(GTK_FILES.map(|name| format!("{dir}/{name}")));
        }
        Ok(files)
    }
    fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("unsupported picker configuration version".into());
        }
        for name in [&self.icons.dark, &self.icons.light] {
            if name.is_empty() || name.chars().any(|c| c.is_control() || ['\'', '"', '\\', '/'].contains(&c)) {
                return Err("invalid icon theme name".into());
            }
        }
        if let Some(themes) = &self.vscode {
            for name in [&themes.dark, &themes.light] {
                if name.trim().is_empty() || name.chars().any(char::is_control) {
                    return Err("VS Code theme names must be nonempty and contain no control characters".into());
                }
            }
        }
        let outputs = self.output_paths()?;
        let set: BTreeSet<_> = outputs.iter().collect();
        if set.len() != outputs.len() {
            return Err("theme output paths must be distinct".into());
        }
        for file in &outputs {
            if outputs.iter().any(|other| other.starts_with(&format!("{file}/"))) {
                return Err("a theme file path conflicts with an output directory".into());
            }
        }
        let inputs = [&self.qt.profiles.dark, &self.qt.profiles.light, &self.gtk.profiles.dark, &self.gtk.profiles.light, &self.gtk.layouts.gtk3, &self.gtk.layouts.gtk4];
        for input in inputs {
            let input = relative(input)?;
            if outputs.iter().any(|out| input == *out || input.starts_with(&format!("{out}/"))) {
                return Err("theme outputs must not overwrite profiles or source layouts".into());
            }
        }
        Ok(())
    }
}
fn validate_backup(paths: &Paths, backup: &Backup) -> Result<(), String> {
    let unique: BTreeSet<_> = backup.files.iter().map(|s| &s.relative).collect();
    if backup.version != 1 || backup.config_root != paths.config || backup.files.is_empty() || unique.len() != backup.files.len() || backup.files.iter().any(|f| !safe_relative(&f.relative) || (f.permissions & !0o777 != 0) || (!f.relative.starts_with("oxidye/") && f.relative != crate::vscode::SETTINGS && !SETTINGS.contains(&f.relative.as_str()))) {
        return Err("invalid combined theme backup or configuration root".into());
    }
    if let Some(p) = &backup.preferences {
        if p.keys().map(String::as_str).collect::<BTreeSet<_>>() != PREFERENCES.into_iter().collect() {
            return Err("invalid theme preference backup".into());
        }
    }
    Ok(())
}
fn remember(paths: &Paths, before: &[Snapshot], prefs: &Option<BTreeMap<String, String>>) -> Result<(), String> {
    let mut backup = if let Some(data) = read(&backup_path(paths))? {
        let b: Backup = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        validate_backup(paths, &b)?;
        b
    } else {
        Backup { version: 1, config_root: paths.config.clone(), files: Vec::new(), preferences: prefs.clone() }
    };
    for file in before {
        if !backup.files.iter().any(|old| old.relative == file.relative) {
            backup.files.push(file.clone());
        }
    }
    if backup.preferences.is_none() {
        backup.preferences = prefs.clone();
    }
    atomic(&backup_path(paths), &json_bytes(&backup)?, 0o600)
}
fn set_ini(plan: &mut BTreeMap<String, String>, file: &str, group: &str, key: &str, value: &str) {
    let updates = BTreeMap::from([(group.into(), BTreeMap::from([(key.into(), value.into())]))]);
    let content = edit_ini(&plan[file], &updates);
    plan.insert(file.into(), content);
}
fn rollback(paths: &Paths, before: &[Snapshot], changed: &[Snapshot]) -> String {
    let names: BTreeSet<_> = changed.iter().map(|s| s.relative.as_str()).collect();
    let original: Vec<_> = before.iter().filter(|s| names.contains(s.relative.as_str())).cloned().collect();
    config_io::transaction(&paths.config, changed, &original, "theme rollback").err().unwrap_or_else(|| "complete".into())
}
pub use crate::gtk::install::validator_directory;

/// Generate only on an explicit accent request. Cache files are written only if their contents change; mode switches select matching existing Qt/GTK files.
pub fn apply(paths: &Paths, mode: Option<Mode>, seed: Option<Seed>, validators: &Path, notify: bool) -> Result<Applied, String> {
    // Serialize application and restoration of the fixed theme.
    let _theme = paths.lock()?;
    let mut config = Config::read(paths)?;
    let selected = mode.unwrap_or(config.mode);
    let accent_path = relative(&config.accents)?;
    let accents = if let Some(seed) = seed { accents_from_bundle(&generate(seed)?) } else { AccentFile::read(&paths.config.join(&accent_path))? };
    let names = config.output_paths()?;
    let before = config_io::capture(&paths.config, &names.iter().map(String::as_str).collect::<Vec<_>>())?;
    let layouts = [fs::read_to_string(paths.config.join(relative(&config.gtk.layouts.gtk3)?)).map_err(|e| e.to_string())?, fs::read_to_string(paths.config.join(relative(&config.gtk.layouts.gtk4)?)).map_err(|e| e.to_string())?];
    let mut plan = BTreeMap::new();
    if let Some(themes) = &config.vscode {
        let input = before.iter().find(|s| s.relative == crate::vscode::SETTINGS).unwrap();
        let settings = crate::vscode::settings(input.contents.as_deref().unwrap_or("{}"), themes, selected)?;
        plan.insert(crate::vscode::SETTINGS.into(), settings);
    }
    let mut active_qt = None;
    let mut active_gtk = None;
    for appearance in [Mode::Dark, Mode::Light] {
        let qt_profile = qt::Profile::read(&paths.config.join(relative(pick(&config.qt.profiles, appearance))?))?;
        let gtk_profile = gtk::Profile::read(&paths.config.join(relative(pick(&config.gtk.profiles, appearance))?))?;
        if qt_profile.mode()? != appearance || gtk_profile.mode != appearance {
            return Err("picker profile mode mismatch".into());
        }
        let qt = qt_profile.compose(&accents)?;
        let gtk = gtk_profile.compose(&accents)?;
        let palette = relative(pick(&config.qt.palettes, appearance))?;
        let scheme = relative(pick(&config.qt.schemes, appearance))?;
        plan.insert(palette.clone(), qt.palette_ini());
        plan.insert(scheme.clone(), qt.scheme_ini());
        let files = gtk.files(&layouts)?;
        let dir = relative(pick(&config.gtk.outputs, appearance))?;
        if appearance == selected {
            for f in qt::install::selection(paths, &qt, &before, &palette, &scheme)? {
                plan.insert(f.relative, f.contents.unwrap());
            }
            for f in gtk::install::selection(paths, &gtk, &files, validators, &before, &dir)? {
                plan.insert(f.relative, f.contents.unwrap());
            }
            active_qt = Some(qt);
            active_gtk = Some(gtk);
        } else {
            let stage = tempfile::tempdir_in(&paths.state).map_err(|e| e.to_string())?;
            for (name, data) in &files {
                fs::write(stage.path().join(name), data).map_err(|e| e.to_string())?;
            }
            for version in [3, 4] {
                gtk::install::native_check(validators, version, &stage.path().join(format!("gtk{version}.css")))?;
            }
            for (name, data) in files {
                plan.insert(format!("{dir}/{name}"), data);
            }
        }
    }
    let qt = active_qt.unwrap();
    let gtk = active_gtk.unwrap();
    let icon = pick(&config.icons, selected).clone();
    set_ini(&mut plan, "kdeglobals", "Icons", "Theme", &icon);
    for version in [5, 6] {
        set_ini(&mut plan, &format!("qt{version}ct/qt{version}ct.conf"), "Appearance", "icon_theme", &icon);
    }
    for version in [3, 4] {
        set_ini(&mut plan, &format!("gtk-{version}.0/settings.ini"), "Settings", "gtk-icon-theme-name", &icon);
    }
    if let Some(input) = plan.get_mut("xsettingsd/xsettingsd.conf") {
        let mut found = false;
        *input = input
            .lines()
            .map(|line| {
                if line.split_whitespace().next() == Some("Net/IconThemeName") {
                    found = true;
                    format!("Net/IconThemeName \"{icon}\"\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect();
        if !found {
            input.push_str(&format!("Net/IconThemeName \"{icon}\"\n"));
        }
    }
    let applied = Applied { version: 1, mode: selected, seed: accents.seed.clone(), named_accent: gtk.named_accent.clone(), icon_theme: icon.clone(), vscode_theme: config.vscode.as_ref().map(|themes| pick(themes, selected).clone()) };
    config.mode = selected;
    let snapshot = |relative: String, contents: String| Snapshot { permissions: before.iter().find(|s| s.relative == relative).unwrap().permissions, relative, contents: Some(contents) };
    let mut tail = Vec::new();
    if seed.is_some() {
        tail.push(snapshot(accent_path, String::from_utf8(json_bytes(&accents)?).unwrap()));
    }
    tail.push(snapshot(APPLIED.into(), String::from_utf8(json_bytes(&applied)?).unwrap()));
    tail.push(snapshot(CONFIG.into(), String::from_utf8(json_bytes(&config)?).unwrap()));
    let files: Vec<_> = plan
        .into_iter()
        .map(|(p, s)| snapshot(p, s))
        .filter(|s| {
            // Rewrite qtct roots to trigger its directory watcher on accent changes.
            s.relative == "qt5ct/qt5ct.conf" || s.relative == "qt6ct/qt6ct.conf" || before.iter().find(|old| old.relative == s.relative).unwrap().contents != s.contents
        })
        .collect();
    let current = if notify { Some(gtk::install::preferences_for(&PREFERENCES)?) } else { None };
    if let Some(original) = before.iter().find(|s| s.relative == crate::vscode::SETTINGS) {
        if read(&paths.config.join(crate::vscode::SETTINGS))? != original.contents {
            return Err("VS Code settings changed while preparing the theme; retry the operation".into());
        }
    }
    remember(paths, &before, &current)?;
    config_io::transaction(&paths.config, &before, &files, "theme")?;
    let theme = if selected == Mode::Dark { "adw-gtk3-dark" } else { "adw-gtk3" };
    let preferences = BTreeMap::from([("gtk-theme".into(), format!("'{theme}'")), ("color-scheme".into(), format!("'prefer-{}'", selected.as_str())), ("accent-color".into(), format!("'{}'", gtk.named_accent)), ("icon-theme".into(), format!("'{icon}'"))]);
    if let Some(current) = &current {
        if let Err(error) = gtk::install::set_preferences(&preferences, current) {
            return Err(format!("{error}; theme file restoration: {}", rollback(paths, &before, &files)));
        }
    }
    if let Err(error) = config_io::transaction(&paths.config, &before, &tail, "theme selection") {
        let prefs = current.as_ref().and_then(|p| gtk::install::set_preferences(p, &preferences).err()).unwrap_or_else(|| "complete".into());
        return Err(format!("{error}; preference restoration: {prefs}; theme file restoration: {}", rollback(paths, &before, &files)));
    }
    if notify {
        let mut groups = qt.kde.clone();
        groups.insert("Icons".into(), BTreeMap::from([("Theme".into(), icon)]));
        qt::install::notify_breeze(&groups);
        gtk::install::notify_xsettings();
    }
    Ok(applied)
}
pub fn restore(paths: &Paths, notify: bool) -> Result<(), String> {
    let _theme = paths.lock()?;
    let backup: Backup = serde_json::from_str(&read(&backup_path(paths))?.ok_or("no combined theme backup has been saved")?).map_err(|e| e.to_string())?;
    validate_backup(paths, &backup)?;
    let names: Vec<_> = backup.files.iter().map(|s| s.relative.as_str()).collect();
    let before = config_io::capture(&paths.config, &names)?;
    let current = if notify && backup.preferences.is_some() { Some(gtk::install::preferences_for(&PREFERENCES)?) } else { None };
    config_io::transaction(&paths.config, &before, &backup.files, "theme restoration")?;
    if let (Some(original), Some(current)) = (&backup.preferences, &current) {
        if let Err(error) = gtk::install::set_preferences(original, current) {
            return Err(format!("{error}; file restoration: {}", rollback(paths, &before, &backup.files)));
        }
    }
    if notify {
        qt::install::notify_breeze(&BTreeMap::new());
        gtk::install::notify_xsettings();
    }
    Ok(())
}
