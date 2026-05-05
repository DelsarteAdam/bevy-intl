# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/).

## [0.3.0] - 2026-05-05

### Added

- `t_with_args(key, &[(name, value)])` — proper **named** placeholder substitution; placeholder names in the JSON are matched against the keys you pass (the previous API silently ignored names and consumed args by position).
- `t_with_gender_and_args` — named-placeholder counterpart for gendered translations.
- `t_with_gender_and_plural(key, gender, count)` — combine gender and CLDR plural in a single call (driven by the new `SectionValue::Nested` JSON shape).
- `try_set_lang` / `try_set_fallback_lang` returning `Result<(), I18nError>`; the public `I18nError` enum (currently `LocaleNotFound(String)`) implements `Display + std::error::Error`.
- `i18n_args!{ name = ..., count = ... }` declarative macro for ergonomic named arg lists.
- **Reactive UI**: `I18nText` component (with `#[require(Text)]`), `I18nMode` enum (`Plain`, `Args`, `Plural`, `Gender`, `GenderArgs`, `GenderPlural`), and an `update_i18n_text` system registered automatically by `I18nPlugin`. The system re-renders all `I18nText` entities when the language changes and incrementally re-renders `Added` / `Changed` ones the rest of the time.
- `LanguageChanged` message broadcast when the active language changes (Bevy 0.18 renamed buffered events to *messages* — read with `MessageReader<LanguageChanged>`).
- CLDR-correct plural rules via `intl_pluralrules`. Polish, Russian, Arabic etc. now resolve `one` / `few` / `many` / `other` correctly per CLDR.
- ISO/CLDR locale validation at startup: warns when a `messages/` folder name is not a recognized locale.
- `I18nConfig.warn_unknown_locales: bool` (default `true`) to silence the above for projects with intentional custom codes.
- Comprehensive unit + integration test suite (placeholder regex, plural fallback, locale validation, fallback-language behaviour, `I18nText` reactivity).

### Changed

- **Breaking** — `I18nPartial` now borrows its sections from `I18n` (zero-copy). Adds a lifetime parameter: `I18nPartial<'_>`. If you stored it in a struct, that struct now needs a lifetime; usually you can inline `i18n.translation("ui")` at the call site.
- **Breaking** — `SectionValue` adds a `Nested(HashMap<String, HashMap<String, String>>)` variant for gender × plural JSON. Order matters for `serde(untagged)`: the new variant is tried before `Map`, so plain string maps still parse as before.
- **Breaking** — `LanguageAppExt` methods now return `&mut Self` so they chain with the `App` builder; the trait is intended for build-time configuration before `app.run()`, not for use inside Bevy systems.
- `available_languages()` now returns an alphabetically-sorted list (was non-deterministic).
- MSRV bumped to **Rust 1.85** (uses `std::sync::LazyLock` and edition 2024).
- Internal field rename: `file_traductions` → `file_translations`, `fallback_traduction` → `fallback_translation` (private fields, no API impact).
- Crate description and various docstrings translated to English.

### Deprecated

- `t_with_arg(key, &[&dyn ToString])` — use `t_with_args` (named tuples or the `i18n_args!` macro) for proper named substitution. The deprecated path still works but uses **positional** order of appearance, ignoring placeholder names.
- `t_with_gender_and_arg` — use `t_with_gender_and_args`.

### Fixed

- **Infinite recursion** when loading translations on WASM with empty bundled data: `load_filesystem_translations` no longer calls `load_bundled_translations` on WASM, and the bundled-data fallback is gated to non-WASM targets.
- `replace_placeholders` produced inconsistent output through `Regex::split`; positional substitution now uses a stable counter via `Regex::replace_all`.
- `LanguageAppExt` example in docs (and README) showed `fn setup_language(mut app: ResMut<App>)`, which never compiled — `App` is not a `Resource`. Replaced with build-time chaining.
- Default and fallback languages are now validated against the loaded translations at startup; a warning is logged if either is missing.
- `t_with_args` (and the new fallible APIs) emit `warn!` when a key is missing or a placeholder name has no matching value, surfacing typos that previously stayed silent.

### Removed

- `once_cell` dependency (replaced by `std::sync::LazyLock`).
- "Hot-reloadable during development" claim from the README — no implementation exists.
- Top-level `#![allow(dead_code)]` from `lib.rs`. The `LOCALES` ISO table is now actively used by the startup locale validation.

[0.3.0]: https://github.com/DelsarteAdam/bevy-intl/releases/tag/v0.3.0
