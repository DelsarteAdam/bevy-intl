//! Reactive translation component and supporting types.
//!
//! Spawn an [`I18nText`] alongside any entity that has a `Text` component;
//! the [`update_i18n_text`] system (registered automatically by
//! [`crate::I18nPlugin`]) keeps the rendered text in sync with the active
//! language. When the language changes, every `I18nText` in the world is
//! re-rendered and a [`LanguageChanged`] event is fired so other systems can
//! react (e.g. reloading localized assets).

use bevy::prelude::*;

use crate::I18n;

/// Component describing a translation key to render into a sibling `Text`.
///
/// The component owns its `file` / `key` strings to keep things `Send + Sync`
/// without lifetimes; for hot UI text consider caching the
/// [`I18nText`] entity rather than rebuilding it every frame.
#[derive(Component, Clone, Debug)]
#[require(Text)]
pub struct I18nText {
    /// Translation file (without the `.json` extension), e.g. `"ui"`.
    pub file: String,
    /// Translation key inside that file, e.g. `"welcome"`.
    pub key: String,
    /// How to render the translation (plain, plural, gender, …).
    pub mode: I18nMode,
}

impl I18nText {
    /// Convenience constructor for a plain translation.
    pub fn new(file: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            key: key.into(),
            mode: I18nMode::Plain,
        }
    }
}

/// Selects which translation method to call when rendering an [`I18nText`].
#[derive(Clone, Debug)]
pub enum I18nMode {
    /// `t(key)`
    Plain,
    /// `t_with_args(key, args)` — owned name/value pairs (any `Display` value).
    Args(Vec<(String, String)>),
    /// `t_with_plural(key, count)`
    Plural(usize),
    /// `t_with_gender(key, gender)`
    Gender(String),
    /// `t_with_gender_and_args(key, gender, args)`
    GenderArgs(String, Vec<(String, String)>),
    /// `t_with_gender_and_plural(key, gender, count)`
    GenderPlural(String, usize),
}

/// Message broadcast by [`update_i18n_text`] when the active language changes.
///
/// Useful for reacting to language changes outside of `I18nText` (e.g. swapping
/// images, reloading audio, refreshing a custom widget). Read it with a
/// `MessageReader<LanguageChanged>` system param.
///
/// Bevy 0.18 renamed buffered events to *messages*, so this type derives
/// `Message` rather than `Event` — the practical usage is the same.
#[derive(Message, Debug, Clone)]
pub struct LanguageChanged {
    pub from: String,
    pub to: String,
}

/// Bevy system that keeps `Text` in sync with `I18nText`.
///
/// - When the active language changes, every `I18nText` is re-rendered and a
///   `LanguageChanged` event is written.
/// - Otherwise, only entities with `Added<I18nText>` or `Changed<I18nText>` are
///   re-rendered (cheap incremental updates on spawn / edit).
pub fn update_i18n_text(
    i18n: Res<I18n>,
    mut sets: ParamSet<(
        Query<(&I18nText, &mut Text), Or<(Changed<I18nText>, Added<I18nText>)>>,
        Query<(&I18nText, &mut Text)>,
    )>,
    mut last_lang: Local<Option<String>>,
    mut events: MessageWriter<LanguageChanged>,
) {
    let current = i18n.get_lang().to_string();
    let lang_changed = last_lang.as_deref() != Some(current.as_str());

    if lang_changed {
        let prev = last_lang.replace(current.clone());
        if let Some(prev) = prev {
            events.write(LanguageChanged { from: prev, to: current.clone() });
        }
        let mut q = sets.p1();
        for (it, mut text) in &mut q {
            text.0 = render(&i18n, it);
        }
    } else {
        let mut q = sets.p0();
        for (it, mut text) in &mut q {
            text.0 = render(&i18n, it);
        }
    }
}

fn render(i18n: &I18n, it: &I18nText) -> String {
    let t = i18n.translation(&it.file);
    match &it.mode {
        I18nMode::Plain => t.t(&it.key),
        I18nMode::Plural(c) => t.t_with_plural(&it.key, *c),
        I18nMode::Gender(g) => t.t_with_gender(&it.key, g),
        I18nMode::Args(args) => {
            let view: Vec<(&str, &dyn ToString)> = args
                .iter()
                .map(|(k, v)| (k.as_str(), v as &dyn ToString))
                .collect();
            t.t_with_args(&it.key, &view)
        }
        I18nMode::GenderArgs(g, args) => {
            let view: Vec<(&str, &dyn ToString)> = args
                .iter()
                .map(|(k, v)| (k.as_str(), v as &dyn ToString))
                .collect();
            t.t_with_gender_and_args(&it.key, g, &view)
        }
        I18nMode::GenderPlural(g, c) => t.t_with_gender_and_plural(&it.key, g, *c),
    }
}
