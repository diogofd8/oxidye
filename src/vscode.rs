//! Preserve JSONC comments/formatting while changing only VS Code theme settings.
use crate::{Mode, Modes};
use jsonc_parser::{
    cst::{CstInputValue, CstRootNode},
    ParseOptions,
};

pub(crate) const SETTINGS: &str = "Code/User/settings.json";

pub(crate) fn settings(input: &str, themes: &Modes<String>, mode: Mode) -> Result<String, String> {
    // VS Code accepts JSON plus comments/trailing commas, not arbitrary JSON5.
    let options = ParseOptions { allow_comments: true, allow_trailing_commas: true, allow_loose_object_property_names: false, allow_missing_commas: false, allow_single_quoted_strings: false, allow_hexadecimal_numbers: false, allow_unary_plus_numbers: false, allow_bare_decimal_point_numbers: false, allow_non_finite_numbers: false, allow_extended_string_escapes: false };
    let root = CstRootNode::parse(input, &options).map_err(|e| format!("invalid VS Code settings: {e}"))?;
    let object = root.object_value_or_create().ok_or("VS Code settings must contain a JSON object")?;
    let selected = match mode {
        Mode::Dark => &themes.dark,
        Mode::Light => &themes.light,
    };
    for (key, value) in [("workbench.colorTheme", selected), ("workbench.preferredDarkColorTheme", &themes.dark), ("workbench.preferredLightColorTheme", &themes.light), ("workbench.preferredHighContrastColorTheme", &themes.dark), ("workbench.preferredHighContrastLightColorTheme", &themes.light)] {
        let properties: Vec<_> = object.properties().into_iter().filter(|p| p.decoded_name().as_deref() == Some(key)).collect();
        if properties.is_empty() {
            object.append(key, CstInputValue::String(value.clone()));
        } else {
            // Update every occurrence: VS Code accepts duplicate setting keys and the last one can otherwise override the selected theme.
            for property in properties {
                let current = property.value().and_then(|v| serde_json::from_str::<String>(&v.to_string()).ok());
                if current.as_ref() != Some(value) {
                    property.set_value(CstInputValue::String(value.clone()));
                }
            }
        }
    }
    Ok(root.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn themes() -> Modes<String> {
        Modes { dark: "GitHub Dark High Contrast".into(), light: "GitHub Light High Contrast".into() }
    }
    #[test]
    fn preserves_jsonc_layout_comments_nested_values_and_unrelated_preferences() {
        let input = "// user preferences\r\n{\r\n\t\"editor.fontSize\": 16, /* keep */\r\n\t\"nested\": {\"workbench.colorTheme\": \"nested\",},\r\n\t\"workbench.colorTheme\": \"old\", // active\r\n}\r\n";
        let light = settings(input, &themes(), Mode::Light).unwrap();
        assert!(light.starts_with("// user preferences\r\n"));
        assert!(light.contains("\t\"editor.fontSize\": 16, /* keep */\r\n"));
        assert!(light.contains("\t\"nested\": {\"workbench.colorTheme\": \"nested\",},\r\n"));
        assert!(light.contains("\"workbench.colorTheme\": \"GitHub Light High Contrast\", // active"));
        assert_eq!(light, settings(&light, &themes(), Mode::Light).unwrap());
        let dark = settings(&light, &themes(), Mode::Dark).unwrap();
        assert!(dark.contains("\"workbench.colorTheme\": \"GitHub Dark High Contrast\", // active"));
    }
    #[test]
    fn rejects_invalid_jsonc_without_replacing_user_data() {
        for input in ["[]", "null", "{\"a\":1 \"b\":2}", "{invalid:true}", "{\"a\":NaN}", "{\"unfinished\":"] {
            assert!(settings(input, &themes(), Mode::Dark).is_err(), "{input}");
        }
        assert!(settings("// new settings", &themes(), Mode::Light).is_ok());
    }
    #[test]
    fn handles_duplicate_keys_unicode_and_escaped_theme_names() {
        let themes = Modes { dark: "Dark \"Contrast\" \\ · 🌒".into(), light: "Light ☀".into() };
        let result = settings(r#"{"workbench.colorTheme":"a","workbench.colorTheme":"b"}"#, &themes, Mode::Dark).unwrap();
        let root = CstRootNode::parse(&result, &ParseOptions::default()).unwrap();
        for prop in root.object_value().unwrap().properties() {
            if prop.decoded_name().as_deref() == Some("workbench.colorTheme") {
                assert_eq!(serde_json::from_str::<String>(&prop.value().unwrap().to_string()).unwrap(), themes.dark);
            }
        }
    }
}
