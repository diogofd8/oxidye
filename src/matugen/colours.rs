//! Map Material roles to complete toolkit colours. Keep imported bytes unless a derived state or contrast repair needs a different tone.
use crate::{contrast, gtk, qt, Mode, Modes, Seed, TypedColor};
use palette::{FromColor, Oklch, Srgb};
use std::collections::BTreeMap;

type Rgb = [u8; 3];
type Colours = BTreeMap<String, gtk::Rgba>;

fn parse(value: &str) -> Result<Rgb, String> {
    let hex = value.strip_prefix('#').filter(|s| s.len() == 6 && s.bytes().all(|c| c.is_ascii_hexdigit())).ok_or_else(|| format!("expected opaque Matugen RGB hex, got {value:?}"))?;
    let channel = |offset| u8::from_str_radix(&hex[offset..offset + 2], 16).map_err(|_| format!("invalid Matugen RGB hex: {value}"));
    Ok([channel(0)?, channel(2)?, channel(4)?])
}
fn seed(rgb: Rgb) -> Seed {
    let [r, g, b] = rgb.map(|c| f64::from(c) / 255.0);
    let colour: Oklch<f64> = Oklch::from_color(Srgb::new(r, g, b));
    Seed { l: colour.l.clamp(0.0, 1.0), c: colour.chroma, h: colour.hue.into_positive_degrees() }
}
fn derive(rgb: Rgb, lightness: f64, chroma: f64) -> Rgb {
    let value = seed(rgb);
    crate::bytes(crate::gamut_map(Seed { l: lightness.clamp(0.0, 1.0), c: value.c * chroma, ..value }))
}
fn get(colours: &Colours, name: &str) -> Rgb {
    colours[name].rgb
}
fn set(colours: &mut Colours, name: &str, rgb: Rgb) {
    colours.insert(name.into(), gtk::Rgba { rgb, alpha: 1.0 });
}
fn nearest(rgb: Rgb, backgrounds: &[Rgb], target: f64) -> Result<Rgb, String> {
    if backgrounds.iter().all(|bg| contrast(rgb, *bg) >= target) {
        return Ok(rgb);
    }
    crate::nearest(seed(rgb), |tone| backgrounds.iter().all(|bg| contrast(crate::bytes(tone), *bg) >= target)).map(crate::bytes).ok_or_else(|| format!("cannot find a readable tone at contrast {target}"))
}
fn repair(colours: &mut Colours, role: &str, backgrounds: &[Rgb], target: f64) -> Result<(), String> {
    let rgb = nearest(get(colours, role), backgrounds, target).map_err(|e| format!("{role}: {e}"))?;
    set(colours, role, rgb);
    Ok(())
}
fn repair_family(colours: &mut Colours, foreground: &str, fills: &[&str]) -> Result<(), String> {
    let fg = get(colours, foreground);
    let backgrounds: Vec<_> = fills.iter().map(|name| get(colours, name)).collect();
    if let Ok(value) = nearest(fg, &backgrounds, 4.5) {
        set(colours, foreground, value);
        return Ok(());
    }
    let mut best: Option<(f64, Rgb, Vec<Rgb>)> = None;
    for candidate in [[0; 3], [255; 3]] {
        let adjusted = backgrounds.iter().map(|bg| nearest(*bg, &[candidate], 4.5)).collect::<Result<Vec<_>, _>>();
        if let Ok(adjusted) = adjusted {
            let cost = (seed(candidate).l - seed(fg).l).abs() + adjusted.iter().zip(&backgrounds).map(|(a, b)| (seed(*a).l - seed(*b).l).abs()).sum::<f64>();
            if best.as_ref().is_none_or(|old| cost < old.0) {
                best = Some((cost, candidate, adjusted));
            }
        }
    }
    let (_, fg, adjusted) = best.ok_or_else(|| format!("cannot resolve {foreground} against its fills"))?;
    set(colours, foreground, fg);
    for (name, value) in fills.iter().zip(adjusted) {
        set(colours, name, value);
    }
    Ok(())
}
const BODY_BACKGROUNDS: [&str; 12] = ["window_bg", "view_bg", "view_alt", "button_bg", "button_hover", "button_pressed", "menu_bg", "menu_hover", "tooltip_bg", "surface_highest", "headerbar_bg", "sidebar_bg"];
pub(super) fn gtk(export: &BTreeMap<String, Modes<TypedColor>>, mode: Mode) -> Result<gtk::Rendered, String> {
    let material = |role: &str| -> Result<Rgb, String> {
        let colour = export.get(role).ok_or_else(|| format!("Matugen output is missing colour role {role}"))?;
        parse(
            &match mode {
                Mode::Dark => &colour.dark,
                Mode::Light => &colour.light,
            }
            .color,
        )
    };
    let mut colours = Colours::new();
    for (role, source) in [("window_bg", "surface"), ("window_fg", "on_surface"), ("view_bg", "surface_container_lowest"), ("view_fg", "on_surface"), ("view_alt", "surface_container_low"), ("surface_low", "surface_container_low"), ("surface", "surface_container"), ("surface_high", "surface_container_high"), ("surface_highest", "surface_container_highest"), ("button_bg", "surface_container_high"), ("button_fg", "on_surface"), ("button_hover", "surface_container_highest"), ("button_pressed", "surface_variant"), ("border", "outline_variant"), ("control_border", "outline"), ("muted_fg", "on_surface_variant"), ("disabled_fg", "outline"), ("disabled_bg", "surface_container"), ("headerbar_bg", "surface_container_low"), ("headerbar_fg", "on_surface"), ("headerbar_backdrop", "surface"), ("sidebar_bg", "surface_container_low"), ("sidebar_fg", "on_surface"), ("sidebar_backdrop", "surface"), ("secondary_sidebar_bg", "surface_container"), ("card_bg", "surface_container"), ("card_fg", "on_surface"), ("dialog_bg", "surface_container_high"), ("dialog_fg", "on_surface"), ("popover_bg", "surface_container_high"), ("popover_fg", "on_surface"), ("menu_bg", "surface_container_low"), ("menu_hover", "surface_container_highest"), ("tooltip_bg", "surface_container_highest"), ("tooltip_fg", "on_surface"), ("thumbnail_bg", "surface_container_high"), ("toggle_bg", "surface_container_highest"), ("toggle_fg", "on_surface"), ("scrollbar", "outline"), ("scrollbar_hover", "on_surface_variant"), ("osd_bg", "surface_container_highest"), ("osd_fg", "on_surface"), ("light", "surface_bright"), ("midlight", "surface_container_highest"), ("dark", "surface_dim"), ("mid", "surface_container"), ("reference", "source_color"), ("accent_bg", "primary"), ("accent_fg", "on_primary"), ("accent_text", "primary"), ("accent_soft", "primary_container"), ("selection_bg", "primary_container"), ("selection_fg", "on_primary_container"), ("selection_inactive_fg", "on_surface"), ("focus", "primary"), ("link", "primary"), ("link_visited", "tertiary")] {
        set(&mut colours, role, material(source)?);
    }
    let accent = get(&colours, "accent_bg");
    let direction = if mode == Mode::Dark { 1.0 } else { -1.0 };
    set(&mut colours, "accent_hover", derive(accent, seed(accent).l + direction * 0.035, 1.0));
    set(&mut colours, "accent_pressed", derive(accent, seed(accent).l + direction * 0.065, 1.0));
    let selected = get(&colours, "selection_bg");
    let inactive_l = (seed(selected).l + seed(get(&colours, "window_bg")).l) * 0.5;
    set(&mut colours, "selection_inactive_bg", derive(selected, inactive_l, 0.65));
    let disabled_l = seed(get(&colours, "window_fg")).l * 0.45 + seed(get(&colours, "window_bg")).l * 0.55;
    set(&mut colours, "disabled_fg", derive(material("on_surface")?, disabled_l, 0.5));
    for (role, alpha) in [("shade", 0.12), ("shade_strong", 0.25), ("shadow", 0.5), ("scrollbar_outline", 0.0)] {
        colours.insert(role.into(), gtk::Rgba { rgb: material("scrim")?, alpha });
    }
    for (role, source) in [("error", "error"), ("destructive", "error"), ("success", "success"), ("warning", "warning"), ("info", "info")] {
        set(&mut colours, &format!("{role}_bg"), material(source)?);
        set(&mut colours, &format!("{role}_fg"), material(&format!("on_{source}"))?);
        set(&mut colours, &format!("{role}_text"), material(source)?);
        repair_family(&mut colours, &format!("{role}_fg"), &[&format!("{role}_bg")])?;
    }
    repair_family(&mut colours, "accent_fg", &["accent_bg", "accent_hover", "accent_pressed"])?;
    repair_family(&mut colours, "selection_fg", &["selection_bg"])?;
    repair_family(&mut colours, "selection_inactive_fg", &["selection_inactive_bg"])?;
    let backgrounds: Vec<_> = BODY_BACKGROUNDS.iter().map(|name| get(&colours, name)).collect();
    for role in ["window_fg", "view_fg", "button_fg", "headerbar_fg", "sidebar_fg", "card_fg", "dialog_fg", "popover_fg", "tooltip_fg", "toggle_fg", "osd_fg", "muted_fg", "accent_text", "link", "link_visited", "error_text", "destructive_text", "success_text", "warning_text", "info_text"] {
        repair(&mut colours, role, &backgrounds, 4.5)?;
    }
    for role in ["focus", "control_border"] {
        repair(&mut colours, role, &backgrounds, 3.0)?;
    }
    let reference = seed(material("source_color")?);
    let named_accent = crate::named_accent(seed(get(&colours, "accent_bg"))).into();
    let mut rendered = gtk::Rendered { version: 1, mode, seed: reference.to_string(), named_accent, colours, contrast_checks: Vec::new() };
    rendered.validate()?;
    validate_boundaries(&rendered)?;
    Ok(rendered)
}
pub(super) fn validate_boundaries(gtk: &gtk::Rendered) -> Result<(), String> {
    if gtk.named_accent != crate::named_accent(seed(get(&gtk.colours, "accent_bg"))) {
        return Err("Matugen named accent does not match its rendered primary colour".into());
    }
    for role in ["focus", "control_border"] {
        for bg in BODY_BACKGROUNDS {
            if contrast(get(&gtk.colours, role), get(&gtk.colours, bg)) + 1e-6 < 3.0 {
                return Err(format!("Matugen {role}/{bg} requires 3:1 composited contrast"));
            }
        }
    }
    Ok(())
}
fn argb(rgb: Rgb) -> String {
    format!("#FF{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}
fn triplet(rgb: Rgb) -> String {
    format!("{},{},{}", rgb[0], rgb[1], rgb[2])
}
pub(super) fn qt(gtk: &gtk::Rendered) -> Result<qt::Rendered, String> {
    let rgb = |role| get(&gtk.colours, role);
    let mut active: BTreeMap<String, String> = [("WindowText", "window_fg"), ("Button", "button_bg"), ("Light", "light"), ("Midlight", "midlight"), ("Dark", "dark"), ("Mid", "mid"), ("Text", "view_fg"), ("BrightText", "accent_fg"), ("ButtonText", "button_fg"), ("Base", "view_bg"), ("Window", "window_bg"), ("Shadow", "shadow"), ("Highlight", "selection_bg"), ("HighlightedText", "selection_fg"), ("Link", "link"), ("LinkVisited", "link_visited"), ("AlternateBase", "view_alt"), ("NoRole", "shadow"), ("ToolTipBase", "tooltip_bg"), ("ToolTipText", "tooltip_fg"), ("PlaceholderText", "muted_fg"), ("Accent", "accent_bg")].into_iter().map(|(qt, role)| (qt.into(), argb(rgb(role)))).collect();
    let mut inactive = active.clone();
    inactive.insert("Highlight".into(), argb(rgb("selection_inactive_bg")));
    inactive.insert("HighlightedText".into(), argb(rgb("selection_inactive_fg")));
    let mut disabled = active.clone();
    for role in ["WindowText", "Text", "ButtonText", "HighlightedText", "Link", "LinkVisited", "ToolTipText", "PlaceholderText", "Accent"] {
        disabled.insert(role.into(), argb(rgb("disabled_fg")));
    }
    disabled.insert("Highlight".into(), argb(rgb("disabled_bg")));
    disabled.insert("Button".into(), argb(rgb("disabled_bg")));
    let mut kde: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for (group, bg, fg, alt) in [("Colors:Window", "window_bg", "window_fg", "surface_low"), ("Colors:View", "view_bg", "view_fg", "view_alt"), ("Colors:Button", "button_bg", "button_fg", "button_hover"), ("Colors:Header", "headerbar_bg", "headerbar_fg", "headerbar_backdrop"), ("Colors:Tooltip", "tooltip_bg", "tooltip_fg", "surface_high"), ("Colors:Complementary", "window_bg", "window_fg", "surface_low"), ("Colors:Selection", "selection_bg", "selection_fg", "selection_bg"), (qt::INACTIVE_SELECTION_GROUP, "selection_inactive_bg", "selection_inactive_fg", "selection_inactive_bg")] {
        let selection = group.starts_with("Colors:Selection");
        let foreground = |role| if selection { fg } else { role };
        kde.insert(group.into(), [("BackgroundNormal", bg), ("BackgroundAlternate", alt), ("ForegroundNormal", fg), ("ForegroundInactive", foreground("muted_fg")), ("ForegroundActive", foreground("accent_text")), ("ForegroundLink", foreground("link")), ("ForegroundVisited", foreground("link_visited")), ("ForegroundNegative", foreground("error_text")), ("ForegroundNeutral", foreground("warning_text")), ("ForegroundPositive", foreground("success_text")), ("DecorationFocus", "focus"), ("DecorationHover", "accent_hover")].into_iter().map(|(key, role)| (key.into(), triplet(rgb(role)))).collect());
    }
    kde.insert("ColorEffects:Disabled".into(), BTreeMap::from([("Color".into(), triplet(rgb("disabled_bg"))), ("ColorAmount".into(), "0".into()), ("ColorEffect".into(), "0".into()), ("ContrastAmount".into(), "0.65".into()), ("ContrastEffect".into(), "1".into()), ("IntensityAmount".into(), "0.1".into()), ("IntensityEffect".into(), "2".into())]));
    kde.insert("ColorEffects:Inactive".into(), BTreeMap::from([("Enable".into(), "false".into()), ("ChangeSelectionColor".into(), "false".into()), ("Color".into(), triplet(rgb("window_bg"))), ("ColorAmount".into(), "0".into()), ("ColorEffect".into(), "0".into()), ("ContrastAmount".into(), "0".into()), ("ContrastEffect".into(), "0".into()), ("IntensityAmount".into(), "0".into()), ("IntensityEffect".into(), "0".into())]));
    let name = match gtk.mode {
        Mode::Dark => "OxidyeMatugenDark",
        Mode::Light => "OxidyeMatugenLight",
    };
    kde.insert("General".into(), BTreeMap::from([("ColorScheme".into(), name.into()), ("AccentColor".into(), triplet(rgb("accent_bg")))]));
    let mut contrast_checks = Vec::new();
    for (group, palette) in [("active", &active), ("inactive", &inactive)] {
        for (fg, bg) in [("WindowText", "Window"), ("Text", "Base"), ("Text", "AlternateBase"), ("ButtonText", "Button"), ("ToolTipText", "ToolTipBase"), ("HighlightedText", "Highlight"), ("PlaceholderText", "Base")] {
            let bytes = |role: &str| parse(&format!("#{}", &palette[role][3..]));
            let foreground = bytes(fg)?;
            let background = bytes(bg)?;
            let composited_min = contrast(foreground, background);
            if composited_min + 1e-6 < 4.5 {
                return Err(format!("Matugen Qt {group}/{fg}/{bg} requires 4.5:1 contrast"));
            }
            let a = crate::luminance(foreground.map(|v| f64::from(v) / 255.0));
            let b = crate::luminance(background.map(|v| f64::from(v) / 255.0));
            contrast_checks.push(qt::ContrastCheck { pair: format!("{group} {fg}/{bg}"), target: 4.5, opaque: (a.max(b) + 0.05) / (a.min(b) + 0.05), composited_min });
        }
    }
    Ok(qt::Rendered { version: 1, mode: gtk.mode, seed: gtk.seed.clone(), palette: BTreeMap::from([("active".into(), std::mem::take(&mut active)), ("inactive".into(), inactive), ("disabled".into(), disabled)]), kde, contrast_checks })
}
