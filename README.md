# bevy-intl

A pragmatic internationalization (i18n) plugin for [Bevy](https://bevyengine.org/) that loads translations from JSON files, supports fallback languages, named placeholders, CLDR-correct plurals, gendered text, and a reactive `I18nText` component for Bevy UI. WASM-friendly out of the box.

---

## Features

- **WASM compatible** — translations are bundled at build time for web targets.
- **Flexible loading** — filesystem on desktop, bundled on WASM, or `bundle-only` everywhere via a feature flag.
- **JSON layout** — one folder per language, one file per "namespace" (e.g. `ui.json`, `menu.json`).
- **Named placeholders** — `{{name}}` substituted by name, with the `i18n_args!` macro for ergonomics.
- **CLDR-correct plurals** — backed by [`intl_pluralrules`](https://crates.io/crates/intl_pluralrules); Polish, Russian, Arabic etc. work as expected.
- **Gendered translations** — single-axis or combined gender × plural via nested JSON.
- **Reactive UI** — drop an `I18nText` component on an entity and it stays in sync as the language changes.
- **Fallback language** — automatic fallback when a key is missing.

---

## Quick start

```toml
[dependencies]
bevy = "0.20"
bevy-intl = "0.4"

# Optional: force bundled translations on every target (e.g. for shipping a single binary)
# bevy-intl = { version = "0.4", features = ["bundle-only"] }
```

```rust,no_run
use bevy::prelude::*;
use bevy_intl::{I18nPlugin, I18nConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Default setup auto-detects WASM vs desktop
        .add_plugins(I18nPlugin::default())
        // Or with custom config:
        // .add_plugins(I18nPlugin::with_config(I18nConfig {
        //     messages_folder: "locales".to_string(),
        //     default_lang: "fr".to_string(),
        //     fallback_lang: "en".to_string(),
        //     ..Default::default()
        // }))
        .run();
}
```

**Version compatibility**

| Bevy   | bevy-intl |
| ------ | --------- |
| 0.20.x | `0.4.x`   |
| 0.18.x | `0.3.x`   |
| 0.17.x | `0.2.2`   |
| 0.16.x | `0.2.1`   |

**MSRV** — Rust 1.85 (uses `std::sync::LazyLock` and edition 2024).

---

## Folder layout

```text
messages/
├── en/
│   ├── ui.json
│   └── menu.json
├── fr/
│   ├── ui.json
│   └── menu.json
└── pl/
    ├── ui.json
    └── menu.json
assets/
src/
```

A folder name that is not a recognized ISO/CLDR locale logs a warning at startup. Disable with `I18nConfig.warn_unknown_locales = false` if you intentionally use custom codes.

---

### Where `messages/` is looked up

An absolute `messages_folder` is used as is. A relative one (the default, `"messages"`) is looked up like Bevy's `AssetPlugin` looks up `assets/`:

1. under `BEVY_ASSET_ROOT`, if set;
2. under `CARGO_MANIFEST_DIR` (set by `cargo run`, so it points to your game);
3. next to the executable;
4. in the current working directory.

The first folder that exists wins.

---

## JSON format

Three shapes are supported per key:

```jsonc
{
    "greeting": "Hello",                                          // plain text
    "farewell": {                                                 // single-axis: gender OR plural
        "male": "Goodbye, sir",
        "female": "Goodbye, ma'am"
    },
    "apples": {
        "0": "No apples",                                         // exact-count beats CLDR category
        "one": "One apple",
        "other": "{{count}} apples"
    },
    "guests": {                                                   // two-axis: gender × plural (nested)
        "male":   { "one": "{{count}} guest (M)", "other": "{{count}} guests (M)" },
        "female": { "one": "{{count}} guest (F)", "other": "{{count}} guests (F)" }
    }
}
```

### Plural-key resolution priority

1. **Exact count** — `"0"`, `"1"`, `"5"`, …
2. **CLDR category for the active locale** — resolved by `intl_pluralrules` (so Polish gets `one`/`few`/`many`/`other`, Russian gets `one`/`few`/`many`/`other` with the right buckets, Arabic gets `zero`/`one`/`two`/`few`/`many`/`other`, etc.).
3. **Anglo-centric fallback** — `"one"` for `count == 1`, `"other"` otherwise.
4. **Last resort** — `"many"`.

---

## API

```rust
use bevy::prelude::*;
use bevy_intl::{I18n, i18n_args};

fn translation_system(i18n: Res<I18n>) {
    let t = i18n.translation("ui");

    // Plain
    let _ = t.t("greeting");

    // Named placeholders
    let _ = t.t_with_args("welcome", i18n_args!{ name = "John", count = 5 });
    // Equivalent without the macro:
    let _ = t.t_with_args("welcome", &[("name", &"John"), ("count", &5)]);

    // Plural
    let _ = t.t_with_plural("apples", 5);

    // Gender (single-axis)
    let _ = t.t_with_gender("farewell", "female");

    // Gender + named placeholders
    let _ = t.t_with_gender_and_args("greeting", "male", i18n_args!{ name = "Smith" });

    // Gender + plural (nested JSON)
    let _ = t.t_with_gender_and_plural("guests", "female", 3);

    // Optional keys: `None` when missing, and no warning
    if let Some(text) = t.try_t("tooltip.optional") { /* show it */ }
    let _ = t.try_t_with_args("welcome", i18n_args!{ name = "John", count = 5 });
    let _ = t.has("apples"); // any shape: text, plural/gender map, nested
}
```

A missing key makes `t`, `t_with_plural`, `t_with_gender`… return a `"Missing …"` text and log a warning. The warning is logged **once per key**, so a UI redrawn every frame (egui…) does not flood the console. For keys that may be missing on purpose, use `try_t` / `try_t_with_args`.

> **Deprecated** — `t_with_arg` and `t_with_gender_and_arg` (positional placeholders) still work but ignore placeholder names in your JSON. Migrate to `t_with_args` / `t_with_gender_and_args` for proper named substitution.

### Switching language

```rust
use bevy::prelude::*;
use bevy_intl::I18n;

fn change_language_system(mut i18n: ResMut<I18n>) {
    // Logging variant:
    i18n.set_lang("fr");

    // Result-returning variant:
    if let Err(e) = i18n.try_set_lang("xx") {
        eprintln!("locale not loaded: {e}");
    }

    let _ = i18n.get_lang();              // current
    let _ = i18n.get_fallback_lang();     // fallback
    let _ = i18n.available_languages();   // sorted list
}
```

`set_lang_i18n` / `set_fallback_lang` are also available on `App` (via `LanguageAppExt`) for setting the language at startup *before* `app.run()`:

```rust,no_run
use bevy::prelude::*;
use bevy_intl::{I18nPlugin, LanguageAppExt};

App::new()
    .add_plugins(I18nPlugin::default())
    .set_lang_i18n("fr")
    .set_fallback_lang("en")
    .run();
```

---

## Reactive UI: `I18nText`

Spawn an `I18nText` next to any text node and it stays in sync — no manual rebuild loop, no boilerplate. When the language changes, every `I18nText` is re-rendered and a `LanguageChanged` message is broadcast.

```rust,no_run
use bevy::prelude::*;
use bevy_intl::{I18nPlugin, I18n, I18nText, I18nMode, LanguageChanged};

fn setup_ui(mut commands: Commands) {
    // I18nText auto-adds a `Text` component thanks to `#[require(Text)]`.
    commands.spawn(I18nText::new("ui", "welcome"));

    commands.spawn(I18nText {
        file: "ui".to_string(),
        key:  "guests".to_string(),
        mode: I18nMode::GenderPlural("female".to_string(), 3),
    });
}

fn switcher(input: Res<ButtonInput<KeyCode>>, mut i18n: ResMut<I18n>) {
    if input.just_pressed(KeyCode::F1) { i18n.set_lang("en"); }
    if input.just_pressed(KeyCode::F2) { i18n.set_lang("fr"); }
}

fn react(mut reader: MessageReader<LanguageChanged>) {
    for ev in reader.read() {
        info!("language switched: {} → {}", ev.from, ev.to);
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, I18nPlugin::default()))
        .add_systems(Startup, setup_ui)
        .add_systems(Update, (switcher, react))
        .run();
}
```

Bevy 0.18 renamed buffered events to *messages*, so `LanguageChanged` derives `Message` and is read with `MessageReader<LanguageChanged>` (not `EventReader`).

---

## WASM / platform behaviour

| Target  | Default loading                                           |
| ------- | --------------------------------------------------------- |
| Desktop | reads `messages/` folder at runtime                       |
| WASM    | uses bundled translations (compiled in by `build.rs`)     |

To force bundled mode on every target — for example, to ship a single binary:

```toml
bevy-intl = { version = "0.4", features = ["bundle-only"] }
```

### Telling the build where `messages/` is

Cargo runs bevy-intl's `build.rs` in bevy-intl's own folder (in `~/.cargo/registry` once installed from crates.io), never in your game, so it cannot find your `messages/` folder on its own. Point it there with the `BEVY_INTL_MESSAGES_DIR` variable, in your game's `.cargo/config.toml`:

```toml
[env]
BEVY_INTL_MESSAGES_DIR = { value = "messages", relative = true }
```

`relative = true` makes the path relative to the folder that contains `.cargo/` (your project root). The build fails with a clear message if the path is not absolute or not a folder. If the variable is unset, nothing is bundled; on WASM or with `bundle-only`, the build then prints a warning. The bundle is rebuilt whenever a file in that folder changes.

---

## Migration 0.2 → 0.3

1. **Placeholders** — replace `t_with_arg(key, &[&"John"])` with `t_with_args(key, i18n_args!{ name = "John" })` (positional API kept but deprecated).
2. **Plurals** — exact-count keys still win. CLDR categories now resolve correctly for the active locale; if you authored translations against the old anglo-centric `3..=10 → "few"` rule, double-check Polish/Russian/Arabic JSON.
3. **Events** — `LanguageChanged` is a `Message` (Bevy 0.18). Use `MessageReader`, not `EventReader`.
4. **Type lifetime** — `I18nPartial` now borrows from `I18n` (`I18nPartial<'_>`). If you stored it in a struct, that struct now needs a lifetime parameter; usually you can just inline `i18n.translation("ui")` at the call site.
5. **Naming** — `file_traductions` / `fallback_traduction` are renamed (private fields, no API impact).
6. **MSRV** — bumped to Rust 1.85 (edition 2024).

---

## License

Dual-licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.
