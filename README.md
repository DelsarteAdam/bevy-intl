# bevy-intl Plugin

A simple internationalization (i18n) plugin for [Bevy](https://bevyengine.org/) to manage translations from JSON files. Supports fallback languages, placeholders, plurals, and gendered translations.

---

## Features

-   Load translations from JSON files organized per language.
-   Automatically detect missing files across languages.
-   Support for:
    -   Basic translation
    -   Placeholders/arguments
    -   Plural forms
    -   Gendered text
-   Fallback language if translation is missing.
-   Bevy plugin integration.

---

## Folder Structure

```
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

## JSON

Each JSON file can contain either simple strings or nested maps for plurals/genders:

```
{
  "greeting": "Hello",
  "farewell": {
    "male": "Goodbye, sir",
    "female": "Goodbye, ma'am"
  },
  "apples": {
    "none": "No apples",
    "one": "One apple",
    "many": "{{count}} apples"
  }
}
```

---

## API Usage

#### Accessing translations

```
let i18n: Res<I18n> = ...; // Bevy resource

// Load a translation file
let text = i18n.translation("test");

// Basic translation
let greeting = text.t("greeting");

// Translation with arguments
let apple_count = text.t_with_arg("apples", &[&5]);

// Plural translation
let plural_text = text.t_with_plurial("apples", 5);

// Gendered translation
let farewell = text.t_with_gender("farewell", "female");

// Gendered translation with arguments
let farewell_with_name = text.t_with_gender_and_arg("farewell", "male", &[&"John"]);

```

#### Changing language

```
let mut i18n: ResMut<I18n> = ...;
let mut app: App ;

app.set_lang("fr");         // Set current language
app.set_fallback_lang("en"); // Set fallback language

//or

i18n.set_lang("en"); // Set current language
i18n.get_lang(); // Get current language
```

---

## Exemple

```
fn spawn_text(mut commands: Commands,mut i18n: ResMut<I18n>) {
let text = i18n.translation("test");

    commands.spawn((
        Text::new(text.t("greeting")),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(5.0),
            right: Val::Px(5.0),
            ..default()
        },
    ));

}

```

---

## Debugging

-   Missing translation files or invalid locales are warned in the console.

-   If a translation is missing, the fallback language will be used, or an "Error missing text" placeholder is returned.

---

## License

This crate is licensed under either of the following, at your option:

-   MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
-   Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, shall be dual licensed as above, without
any additional terms or conditions.
