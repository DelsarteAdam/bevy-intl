//! End-to-end test: spin up a Bevy `App` with `I18nPlugin`, point it at a
//! tempdir of fixture JSON, and verify that translations resolve correctly.

use std::fs;

use bevy::prelude::*;
use bevy_intl::{I18n, I18nConfig, I18nPlugin, i18n_args};
use tempfile::tempdir;

fn write_fixture(dir: &std::path::Path, lang: &str, file: &str, content: &str) {
    let lang_dir = dir.join(lang);
    fs::create_dir_all(&lang_dir).unwrap();
    fs::write(lang_dir.join(format!("{}.json", file)), content).unwrap();
}

#[test]
fn loads_translations_from_disk_and_resolves_keys() {
    let temp = tempdir().unwrap();
    write_fixture(
        temp.path(),
        "en",
        "ui",
        r#"{
            "greeting": "Hello",
            "welcome": "Hi {{name}}, you have {{count}} messages",
            "guests": {
                "male":   { "one": "{{count}} guest (M)", "other": "{{count}} guests (M)" },
                "female": { "one": "{{count}} guest (F)", "other": "{{count}} guests (F)" }
            }
        }"#,
    );
    write_fixture(
        temp.path(),
        "fr",
        "ui",
        r#"{
            "greeting": "Bonjour"
        }"#,
    );

    let messages = temp.path().to_string_lossy().into_owned();

    let mut app = App::new();
    app.add_plugins(I18nPlugin::with_config(I18nConfig {
        use_bundled_translations: false,
        messages_folder: messages,
        default_lang: "fr".into(),
        fallback_lang: "en".into(),
        warn_unknown_locales: true,
    }));

    let i18n = app.world().resource::<I18n>();

    // fr override
    assert_eq!(i18n.translation("ui").t("greeting"), "Bonjour");

    // fallback to en when fr is missing the key
    let t = i18n.translation("ui");
    assert_eq!(
        t.t_with_args("welcome", i18n_args!{ name = "Jean", count = 3 }),
        "Hi Jean, you have 3 messages",
    );

    // gender + plural via the nested JSON shape
    assert_eq!(
        t.t_with_gender_and_plural("guests", "female", 2),
        "2 guests (F)"
    );

    // available_languages is alphabetically sorted
    let langs: Vec<&str> = i18n.available_languages().iter().map(String::as_str).collect();
    assert_eq!(langs, vec!["en", "fr"]);
}

#[test]
fn missing_messages_folder_falls_back_to_error_translations() {
    let mut app = App::new();
    app.add_plugins(I18nPlugin::with_config(I18nConfig {
        use_bundled_translations: false,
        messages_folder: "this-path-does-not-exist-xyz".into(),
        default_lang: "en".into(),
        fallback_lang: "en".into(),
        warn_unknown_locales: false,
    }));

    let i18n = app.world().resource::<I18n>();
    // Falls back to a single "en" with one "error" file/key.
    let langs: Vec<&str> = i18n.available_languages().iter().map(String::as_str).collect();
    assert_eq!(langs, vec!["en"]);
}
