//! Qt settings preparation and native Breeze reload notifications.
use super::{edit_ini, Rendered, INACTIVE_SELECTION_GROUP};
use crate::{config_io::Snapshot, Mode, Paths};
use std::{collections::BTreeMap, process::Command};

fn remove_ini_group(input: &str, name: &str) -> String {
    let mut skip = false;
    input
        .split_inclusive('\n')
        .filter(|line| {
            let trimmed = line.trim();
            if let Some(group) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                skip = group == name;
            }
            !skip
        })
        .collect()
}
/// Build the shared Qt selection/settings plan without installing or notifying.
pub(crate) fn selection(paths: &Paths, rendered: &Rendered, before: &[Snapshot], palette_relative: &str, scheme_relative: &str, report: &str) -> Result<Vec<Snapshot>, String> {
    let previous = |name: &str| before.iter().find(|file| file.relative == name).unwrap();
    let mut after = Vec::new();
    let mut push = |relative: &str, contents: String| {
        after.push(Snapshot { relative: relative.into(), contents: Some(contents), permissions: previous(relative).permissions });
    };
    push(palette_relative, rendered.palette_ini());
    push(scheme_relative, rendered.scheme_ini());
    let mut kde_input = previous("kdeglobals").contents.clone().unwrap_or_default();
    if rendered.mode == Mode::Dark {
        // The dark scheme keeps its approved native inactive-selection effect. Remove the light scheme's explicitly owned subgroup on mode changes.
        kde_input = remove_ini_group(&kde_input, INACTIVE_SELECTION_GROUP);
    }
    push("kdeglobals", edit_ini(&kde_input, &rendered.kde));
    for version in [5, 6] {
        let file = format!("qt{version}ct/qt{version}ct.conf");
        let palette_path = paths.config.join(palette_relative);
        let palette_path = palette_path.to_str().ok_or("configuration path is not valid UTF-8")?;
        if palette_path.contains(['\n', '\r', '\0']) {
            return Err("invalid configuration directory".into());
        }
        let updates = BTreeMap::from([("Appearance".into(), BTreeMap::from([("style".into(), "Breeze".into()), ("custom_palette".into(), "true".into()), ("color_scheme_path".into(), palette_path.into())]))]);
        // Replacing the root config also triggers qtct's directory watcher when only the palette in a different directory changed.
        push(&file, edit_ini(previous(&file).contents.as_deref().unwrap_or(""), &updates));
    }
    push(report, String::from_utf8(rendered.json()?).unwrap());
    Ok(after)
}
pub(crate) fn notify_breeze(groups: &BTreeMap<String, BTreeMap<String, String>>) {
    let group_names: Vec<_> = if groups.is_empty() { vec!["General", "Colors:View", "Colors:Window", "Colors:Selection", "Icons"] } else { groups.keys().map(String::as_str).collect() };
    let entries = group_names
        .iter()
        .map(|group| {
            let fallback = if *group == "General" {
                vec!["ColorScheme", "AccentColor"]
            } else if *group == "Icons" {
                vec!["Theme"]
            } else {
                vec!["DecorationFocus", "DecorationHover", "BackgroundNormal", "ForegroundNormal"]
            };
            let keys: Vec<_> = groups.get(*group).map(|values| values.keys().map(String::as_str).collect()).unwrap_or(fallback);
            let arrays = keys.iter().map(|key| format!("@ay [{}]", key.as_bytes().iter().map(u8::to_string).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(", ");
            // KConfig uses a group-separator character for nested groups on D-Bus.
            let wire_group = group.replace("][", "\\u001d");
            format!("'{wire_group}': [{arrays}]")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let changes = format!("@a{{saay}} {{{entries}}}");
    let messages: Vec<(&str, &str, Vec<&str>)> = vec![("/kdeglobals", "org.kde.kconfig.notify.ConfigChanged", vec![&changes]), ("/KGlobalSettings", "org.kde.KGlobalSettings.notifyChange", vec!["0", "0"]), ("/BreezeStyle", "org.kde.Breeze.Style.reparseConfiguration", vec![])];
    for (path, signal, args) in messages {
        match Command::new("gdbus").args(["emit", "--session", "--object-path", path, "--signal", signal]).args(args).output() {
            Ok(result) if result.status.success() => {}
            Ok(result) => eprintln!("oxidye: Qt files installed; reload notification {signal} failed: {}", String::from_utf8_lossy(&result.stderr).trim()),
            Err(error) => {
                eprintln!("oxidye: Qt files installed; native reload unavailable: {error}. Reopen Qt applications.");
                break;
            }
        }
    }
}
