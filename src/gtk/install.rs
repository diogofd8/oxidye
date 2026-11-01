//! GTK settings preparation, native CSS validation and reload notifications.
use super::Rendered;
use crate::{
    config_io::Snapshot,
    qt::{edit_ini, json_bytes},
    Mode, Paths,
};
use std::{
    collections::BTreeMap,
    env, fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};

const SCHEMA: &str = "org.gnome.desktop.interface";
pub fn validator_directory(override_path: Option<PathBuf>) -> Result<PathBuf, String> {
    override_path.or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/libexec/oxidye"))).ok_or_else(|| "set HOME or provide --validator-dir".into())
}
fn gsettings(action: &str, key: &str, value: Option<&str>) -> Result<String, String> {
    let mut command = Command::new("gsettings");
    command.args([action, SCHEMA, key]);
    if let Some(value) = value {
        command.arg(value);
    }
    let output = command.output().map_err(|e| format!("cannot run gsettings: {e}"))?;
    if !output.status.success() {
        return Err(format!("gsettings {action} {key}: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}
pub(crate) fn preferences_for(keys: &[&str]) -> Result<BTreeMap<String, String>, String> {
    keys.iter()
        .copied()
        .map(|key| {
            if gsettings("writable", key, None)? != "true" {
                return Err(format!("GSettings {key} is not writable"));
            }
            Ok((key.into(), gsettings("get", key, None)?))
        })
        .collect()
}
pub(crate) fn set_preferences(after: &BTreeMap<String, String>, before: &BTreeMap<String, String>) -> Result<(), String> {
    let mut changed = Vec::new();
    for (key, value) in after {
        if let Err(error) = gsettings("set", key, Some(value)) {
            let failures: Vec<_> = changed.into_iter().filter_map(|key: &str| gsettings("set", key, Some(&before[key])).err()).collect();
            return Err(format!("{error}; preference rollback errors: {}", failures.join("; ")));
        }
        changed.push(key.as_str());
    }
    Ok(())
}
fn quoted_path(path: &Path) -> Result<String, String> {
    let value = path.to_str().ok_or("CSS path is not valid UTF-8")?;
    if value.contains(['\n', '\r', '\0']) {
        return Err("invalid CSS import path".into());
    }
    Ok(value.replace('\\', "\\\\").replace('"', "\\\""))
}
fn user_css(input: &str) -> String {
    input.lines().filter(|line| !(line.trim().starts_with("@import") && line.contains("oxidye: managed"))).map(|line| format!("{line}\n")).collect()
}
pub(crate) fn native_check(validators: &Path, version: u8, file: &Path) -> Result<(), String> {
    let binary = validators.join(format!("gtk-css-check-{version}"));
    let output = Command::new(&binary).env("GIO_USE_VFS", "local").arg(file).output().map_err(|e| format!("{}: {e}; build/install the GTK CSS checkers first", binary.display()))?;
    if !output.status.success() {
        return Err(format!("GTK {version} CSS validation failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(())
}
pub(crate) fn notify_xsettings() {
    if let Ok(metadata) = fs::metadata("/proc/self") {
        match Command::new("pkill").args(["-HUP", "-x", "-u", &metadata.uid().to_string(), "xsettingsd"]).output() {
            Ok(output) if output.status.success() || output.status.code() == Some(1) => {}
            Ok(output) => eprintln!("GTK files installed; xsettingsd reload failed: {}", String::from_utf8_lossy(&output.stderr).trim()),
            Err(error) => {
                eprintln!("GTK files installed; xsettingsd notification unavailable: {error}")
            }
        }
    }
}
/// Stage and validate GTK output and preserved user rules, then prepare settings.
pub(crate) fn selection(paths: &Paths, rendered: &Rendered, files: &BTreeMap<String, String>, validators: &Path, before: &[Snapshot], directory: &str, report: &str) -> Result<Vec<Snapshot>, String> {
    let stage = tempfile::tempdir_in(&paths.state).map_err(|e| e.to_string())?;
    for (name, data) in files {
        fs::write(stage.path().join(name), data).map_err(|e| e.to_string())?;
    }
    for version in [3, 4] {
        native_check(validators, version, &stage.path().join(format!("gtk{version}.css")))?;
    }
    let previous = |relative: &str| before.iter().find(|s| s.relative == relative).unwrap();
    let mut after = Vec::new();
    let mut push = |relative: String, data: String| after.push(Snapshot { permissions: previous(&relative).permissions, relative, contents: Some(data) });
    for (name, data) in files {
        push(format!("{directory}/{name}"), data.clone());
    }
    let dark = rendered.mode == Mode::Dark;
    let theme = if dark { "adw-gtk3-dark" } else { "adw-gtk3" };
    for version in [3, 4] {
        let relative = format!("gtk-{version}.0/gtk.css");
        let remaining = user_css(previous(&relative).contents.as_deref().unwrap_or(""));
        let target = paths.config.join(format!("{directory}/gtk{version}.css"));
        push(relative, format!("@import url(\"{}\"); /* oxidye: managed */\n{remaining}", quoted_path(&target)?));
        // Validate preserved user rules relative to their original directory.
        let parent = paths.config.join(format!("gtk-{version}.0"));
        fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
        let temporary = tempfile::NamedTempFile::new_in(&parent).map_err(|e| e.to_string())?;
        let validate_css = format!("@import url(\"{}\");\n{remaining}", quoted_path(&stage.path().join(format!("gtk{version}.css")))?);
        fs::write(temporary.path(), validate_css).map_err(|e| e.to_string())?;
        native_check(validators, version, temporary.path())?;
        let settings = format!("gtk-{version}.0/settings.ini");
        let updates = BTreeMap::from([("Settings".into(), BTreeMap::from([("gtk-theme-name".into(), if version == 3 { theme.into() } else { "Adwaita".into() }), ("gtk-application-prefer-dark-theme".into(), if dark { "1".into() } else { "0".into() })]))]);
        push(settings.clone(), edit_ini(previous(&settings).contents.as_deref().unwrap_or(""), &updates));
    }
    if let Some(input) = previous("xsettingsd/xsettingsd.conf").contents.as_deref() {
        let mut found = false;
        let mut output: String = input
            .lines()
            .map(|line| {
                if line.split_whitespace().next() == Some("Net/ThemeName") {
                    found = true;
                    format!("Net/ThemeName \"{theme}\"\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect();
        if !found {
            output.push_str(&format!("Net/ThemeName \"{theme}\"\n"));
        }
        push("xsettingsd/xsettingsd.conf".into(), output);
    }
    push(report.into(), String::from_utf8(json_bytes(&rendered)?).unwrap());
    Ok(after)
}
