//! Minimal example: spawn a couple of `I18nText` entities and switch between
//! `en` and `fr` with F1 / F2. Requires a `messages/{en,fr}/ui.json` next to
//! the working directory (a fixture is provided under `messages/` in the
//! repository).

use bevy::prelude::*;
use bevy_intl::{I18n, I18nMode, I18nPlugin, I18nText, LanguageChanged};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(I18nPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, (language_switcher, react_to_language_change))
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.spawn((
        I18nText::new("ui", "greeting"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(20.0),
            left: Val::Px(20.0),
            ..default()
        },
    ));

    commands.spawn((
        I18nText {
            file: "ui".into(),
            key: "guests".into(),
            mode: I18nMode::GenderPlural("female".into(), 3),
        },
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(60.0),
            left: Val::Px(20.0),
            ..default()
        },
    ));
}

fn language_switcher(input: Res<ButtonInput<KeyCode>>, mut i18n: ResMut<I18n>) {
    if input.just_pressed(KeyCode::F1) {
        i18n.set_lang("en");
    }
    if input.just_pressed(KeyCode::F2) {
        i18n.set_lang("fr");
    }
}

fn react_to_language_change(mut reader: MessageReader<LanguageChanged>) {
    for msg in reader.read() {
        info!("language: {} → {}", msg.from, msg.to);
    }
}
