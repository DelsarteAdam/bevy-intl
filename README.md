# bevy-intl Plugin

A simple internationalization (i18n) plugin for [Bevy](https://bevyengine.org/) to manage translations from JSON files. Supports fallback languages, placeholders, plurals, gendered translations, and **full WASM compatibility** with bundled translations.

---

## Features

-   **🌐 WASM Compatible**: Automatically bundles translations for web deployment
-   **📁 Flexible Loading**: Load from filesystem (desktop) or bundled files (WASM)
-   **🔧 Feature Flag**: `bundle-only` feature to force bundled translations on any platform
-   **🗂️ JSON Organization**: Load translations from JSON files organized per language
-   **🔄 Translation Support**:
    -   Basic translation
    -   Placeholders/arguments
    -   Plural forms
    -   Gendered text
-   **🛡️ Fallback Language**: Automatic fallback when translations are missing
-   **⚡ Bevy Integration**: Native Bevy plugin with resource system integration

---

## 🚀 Quick Setup

Add to your `Cargo.toml`:

```toml
[dependencies]
bevy = "0.16"
bevy-intl = "0.2.0"

# Optional: Force bundled translations on all platforms
# bevy-intl = { version = "0.2.0", features = ["bundle-only"] }
```

Initialize the plugin in your Bevy app:

```rust
use bevy::prelude::*;
use bevy_intl::{I18nPlugin, I18nConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Default setup - auto-detects WASM vs desktop
        .add_plugins(I18nPlugin::default())
        .add_systems(Startup, setup_ui)
        .run();
}

// Or with custom configuration
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(I18nPlugin::with_config(I18nConfig {
            use_bundled_translations: false, // Force filesystem loading  This gets ignored when bundle-only feature is enabled
            messages_folder: "locales".to_string(), // Custom folder
            default_lang: "fr".to_string(),
            fallback_lang: "en".to_string(),
        }))
        .add_systems(Startup, setup_ui)
        .run();
}
```

---

## 📁 Folder Structure

```text
messages/
├── en/
│   ├── test.json
│   └── another_file.json
├── fr/
│   ├── test.json
│   └── another_file.json
└── es/
    ├── test.json
    └── another_file.json
assets/
src/
```

---

## 🌐 WASM & Platform Behavior

**Desktop/Native:**

-   Loads translations from `messages/` folder at runtime
-   Hot-reloadable during development
-   File system access required

**WASM/Web:**

-   Automatically uses bundled translations (compiled at build time)
-   No file system access needed

**Force Bundled Mode:**

```toml
bevy-intl = { version = "0.2.0", features = ["bundle-only"] }
```

This forces bundled translations on all platforms, useful for:

---

## 📄 JSON Format

Each JSON file can contain either simple strings or nested maps for plurals/genders:

```json
{
    "greeting": "Hello",
    "farewell": {
        "male": "Goodbye, sir",
        "female": "Goodbye, ma'am"
    },
    "apples": {
        "zero": "No apples",
        "one": "One apple",
        "two": "A couple of apples",
        "few": "A few apples",
        "many": "{{count}} apples",
        "other": "{{count}} apples"
    },
    "items": {
        "0": "No items",
        "1": "One item",
        "2": "Two items",
        "5": "Exactly five items",
        "other": "{{count}} items"
    }
}
```

### Plural Key Priority (most specific to least):

1. **Exact count**: `"0"`, `"1"`, `"2"`, `"5"`, etc.
2. **ICU Categories**: `"zero"`, `"one"`, `"two"`, `"few"`, `"many"`
3. **Basic fallback**: `"one"` vs `"other"`
4. **Legacy**: `"many"` as last resort

This supports complex plural rules for languages like:

-   **English**: `one`, `other`
-   **French**: `one`, `many`
-   **Polish**: `one`, `few`, `many`
-   **Russian**: `one`, `few`, `many`
-   **Arabic**: `zero`, `one`, `two`, `few`, `many`

---

## 🔧 API Usage

#### Accessing translations in systems

```rust
use bevy::prelude::*;
use bevy_intl::{I18n, LanguageAppExt};

fn translation_system(i18n: Res<I18n>) {
    // Load a translation file
    let text = i18n.translation("test");

    // Basic translation
    let greeting = text.t("greeting");

    // Translation with arguments
    let apple_count = text.t_with_arg("apples", &[&5]);

    // Plural translation
    let plural_text = text.t_with_plural("apples", 5);

    // Gendered translation
    let farewell = text.t_with_gender("farewell", "female");

    // Gendered translation with arguments
    let farewell_with_name = text.t_with_gender_and_arg("farewell", "male", &[&"John"]);
}
```

#### Changing language

```rust
// Method 1: Using App extension trait
fn setup_language(mut app: ResMut<App>) {
    app.set_lang_i18n("fr");         // Set current language
    app.set_fallback_lang("en");     // Set fallback language
}

// Method 2: Direct resource access
fn change_language_system(mut i18n: ResMut<I18n>) {
    i18n.set_lang("en");     // Set current language
    let current = i18n.get_lang(); // Get current language
    let available = i18n.available_languages(); // Get all available languages
}
```

---

## 💡 Complete Example

```rust
use bevy::prelude::*;
use bevy_intl::{I18nPlugin, I18n, LanguageAppExt};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(I18nPlugin::default())
        .add_systems(Startup, (setup_ui, setup_language))
        .add_systems(Update, language_switcher)
        .run();
}

fn setup_language(mut app: ResMut<App>) {
    app.set_lang_i18n("en");
    app.set_fallback_lang("en");
}

fn setup_ui(mut commands: Commands, i18n: Res<I18n>) {
    let text = i18n.translation("ui");

    commands.spawn((
        Text::new(text.t("welcome_message")),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(5.0),
            right: Val::Px(5.0),
            ..default()
        },
    ));
}

fn language_switcher(
    input: Res<ButtonInput<KeyCode>>,
    mut i18n: ResMut<I18n>
) {
    if input.just_pressed(KeyCode::F1) {
        i18n.set_lang("en");
    }
    if input.just_pressed(KeyCode::F2) {
        i18n.set_lang("fr");
    }
}
```

---

## Debugging

-   Missing translation files or invalid locales are warned in the console.

-   If a translation is missing, the fallback language will be used, or an "Error missing text" placeholder is returned.

---

## License

This crate is licensed under either of the following, at your option:

-   MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)
-   Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, shall be dual licensed as above, without
any additional terms or conditions.
