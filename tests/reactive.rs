//! Test that `I18nText` updates and `LanguageChanged` is broadcast when the
//! active language changes.

use std::fs;

use bevy::prelude::*;
use bevy_intl::{I18n, I18nConfig, I18nMode, I18nPlugin, I18nText, LanguageChanged};
use tempfile::tempdir;

fn write_fixture(dir: &std::path::Path, lang: &str, file: &str, content: &str) {
    let lang_dir = dir.join(lang);
    fs::create_dir_all(&lang_dir).unwrap();
    fs::write(lang_dir.join(format!("{}.json", file)), content).unwrap();
}

#[derive(Resource, Default)]
struct CapturedLanguageChanges(Vec<(String, String)>);

fn capture_language_changes(
    mut reader: MessageReader<LanguageChanged>,
    mut log: ResMut<CapturedLanguageChanges>,
) {
    for msg in reader.read() {
        log.0.push((msg.from.clone(), msg.to.clone()));
    }
}

#[test]
fn i18n_text_updates_on_language_change_and_emits_message() {
    let temp = tempdir().unwrap();
    write_fixture(temp.path(), "en", "ui", r#"{ "greeting": "Hello" }"#);
    write_fixture(temp.path(), "fr", "ui", r#"{ "greeting": "Bonjour" }"#);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(I18nPlugin::with_config(I18nConfig {
        use_bundled_translations: false,
        messages_folder: temp.path().to_string_lossy().into_owned(),
        default_lang: "en".into(),
        fallback_lang: "en".into(),
        warn_unknown_locales: false,
    }));
    app.init_resource::<CapturedLanguageChanges>();
    app.add_systems(Update, capture_language_changes);

    let entity = app
        .world_mut()
        .spawn(I18nText {
            file: "ui".into(),
            key: "greeting".into(),
            mode: I18nMode::Plain,
        })
        .id();

    // First update — initial render produces the English text.
    app.update();
    assert_eq!(app.world().get::<Text>(entity).unwrap().0, "Hello");

    // Switch to French. We run two frames: the first one re-renders the
    // `I18nText` and writes the `LanguageChanged` message, the second one
    // gives `capture_language_changes` (which reads after `update_i18n_text`
    // wrote, in system-order terms) a chance to drain the message buffer.
    app.world_mut().resource_mut::<I18n>().set_lang("fr");
    app.update();
    assert_eq!(app.world().get::<Text>(entity).unwrap().0, "Bonjour");
    app.update();

    // The capture system should have observed the en → fr change.
    let captured = app.world().resource::<CapturedLanguageChanges>();
    assert!(
        captured.0.iter().any(|(f, t)| f == "en" && t == "fr"),
        "expected an en → fr LanguageChanged, got {:?}",
        captured.0
    );
}
