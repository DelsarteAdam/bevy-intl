#![doc = include_str!("../README.md")]

//! # bevy-intl
//!
//! A comprehensive internationalization (i18n) plugin for [Bevy](https://bevyengine.org/) that provides:
//! 
//! - **WASM Compatible**: Automatic translation bundling for web deployment
//! - **Flexible Loading**: Filesystem (desktop) or bundled files (WASM)
//! - **Feature Flag**: `bundle-only` to force bundled translations on any platform
//! - **Advanced Plurals**: Support for complex plural rules (ICU-compliant)
//! - **Gender Support**: Gendered translations
//! - **Placeholders**: Dynamic text replacement
//! - **Fallback System**: Automatic fallback to default language
//! 
//! ## Quick Start
//! 
//! ```rust
//! use bevy::prelude::*;
//! use bevy_intl::I18nPlugin;
//! 
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(I18nPlugin::default())
//!         .add_systems(Startup, setup_ui)
//!         .run();
//! }
//! 
//! fn setup_ui(mut commands: Commands, i18n: Res<bevy_intl::I18n>) {
//!     let text = i18n.translation("ui");
//!     
//!     commands.spawn((
//!         Text::new(text.t("welcome")),
//!         Node::default(),
//!     ));
//! }
//! ```
//!
//! ## Features
//!
//! ### Translation Loading
//! - **Desktop**: Loads from `messages/` folder at runtime
//! - **WASM**: Uses bundled translations (compiled at build time)
//! - **Bundle-only**: Force bundled mode with `features = ["bundle-only"]`
//!
//! ### Advanced Plural Support
//! Supports multiple plural forms with fallback priority:
//! 1. Exact counts: `"0"`, `"1"`, `"2"`, etc.
//! 2. ICU categories: `"zero"`, `"one"`, `"two"`, `"few"`, `"many"`
//! 3. Basic fallback: `"one"` vs `"other"`
//!
//! Perfect for complex languages like Polish, Russian, and Arabic.

use bevy::prelude::*;

mod components;
mod locales;

pub use components::{I18nMode, I18nText, LanguageChanged, update_i18n_text};

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;
use serde_json::Value;
use locales::LOCALES;
use regex::Regex;
use intl_pluralrules::{PluralRules, PluralRuleType, PluralCategory};
use unic_langid::LanguageIdentifier;

/// Build an argument slice for the named-placeholder translation methods.
///
/// Expands `i18n_args!{ name = "John", count = 5 }` into the slice form
/// expected by [`I18nPartial::t_with_args`] and
/// [`I18nPartial::t_with_gender_and_args`].
///
/// # Example
///
/// ```rust
/// # use bevy_intl::{i18n_args};
/// // Equivalent to: &[("name", &"John" as &dyn ToString), ("count", &5 as &dyn ToString)]
/// let _ = i18n_args!{ name = "John", count = 5 };
/// ```
#[macro_export]
macro_rules! i18n_args {
    () => { &[] as &[(&str, &dyn ::std::string::ToString)] };
    ($($name:ident = $value:expr),+ $(,)?) => {
        &[$( (stringify!($name), &$value as &dyn ::std::string::ToString) ),+]
            as &[(&str, &dyn ::std::string::ToString)]
    };
}

/// Configuration for the I18n plugin.
/// 
/// Controls how translations are loaded and which languages to use.
/// 
/// # Example
/// 
/// ```rust
/// use bevy_intl::I18nConfig;
/// 
/// let config = I18nConfig {
///     use_bundled_translations: false,
///     messages_folder: "locales".to_string(),
///     default_lang: "fr".to_string(),
///     fallback_lang: "en".to_string(),
/// };
/// ```
#[derive(Debug, Clone, Resource)]
pub struct I18nConfig {
    /// Whether to use bundled translations (true) or filesystem loading (false).
    /// Automatically set to `true` for WASM targets or when `bundle-only` feature is enabled.
    pub use_bundled_translations: bool,
    /// Path to the messages folder containing translation files.
    /// Default: "messages"
    pub messages_folder: String,
    /// Default language code to use.
    /// Default: "en"
    pub default_lang: String,
    /// Fallback language code when a translation is missing.
    /// Default: "en"
    pub fallback_lang: String,
    /// Whether to warn when a folder name in the messages directory is not a
    /// recognized ISO/CLDR locale code. Default: `true`.
    ///
    /// Useful to disable when intentionally using non-standard locale codes
    /// (e.g. "test", "debug", custom dialects).
    pub warn_unknown_locales: bool,
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            use_bundled_translations: cfg!(target_arch = "wasm32") || cfg!(feature = "bundle-only"),
            messages_folder: "messages".to_string(),
            default_lang: "en".to_string(),
            fallback_lang: "en".to_string(),
            warn_unknown_locales: true,
        }
    }
}

// ---------- Bevy Plugin ----------

/// Main plugin for Bevy internationalization.
///
/// Handles language switching, loading translation files, and providing
/// `I18n` resource for accessing localized strings.
///
/// # Example
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_intl::{I18nPlugin, I18nConfig};
///
/// // Default configuration
/// App::new().add_plugins(I18nPlugin::default());
///
/// // Custom configuration
/// App::new().add_plugins(I18nPlugin::with_config(I18nConfig {
///     default_lang: "fr".to_string(),
///     fallback_lang: "en".to_string(),
///     ..Default::default()
/// }));
/// ```
#[derive(Default)]
pub struct I18nPlugin {
    /// Configuration for the plugin
    pub config: I18nConfig,
}

impl I18nPlugin {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: I18nConfig) -> Self {
        Self { config }
    }
}

impl Plugin for I18nPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone())
            .init_resource::<I18n>()
            .add_message::<LanguageChanged>()
            .add_systems(Update, update_i18n_text);
    }
}

/// Represents a value in a translation file.
/// 
/// Can be either a simple text string or a nested map for plurals/genders.
/// 
/// # Examples
/// 
/// Simple text:
/// ```json
/// "greeting": "Hello"
/// ```
/// 
/// Nested map for plurals:
/// ```json
/// "items": {
///   "one": "One item",
///   "many": "{{count}} items"
/// }
/// ```
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum SectionValue {
    /// A simple text value
    Text(String),
    /// A two-level nested map for combining gender and plural (or any other
    /// two-axis discriminator), e.g. `{ "male": { "one": "...", "few": "..." } }`.
    /// `untagged` deserialization tries this variant before [`Self::Map`], so
    /// pure-string objects fall through to `Map` automatically.
    Nested(HashMap<String, HashMap<String, String>>),
    /// A single-level map of key-value pairs (for plurals OR genders alone)
    Map(HashMap<String, String>),
}

/// A mapping of translation keys to their values within a file.
type SectionMap = HashMap<String, SectionValue>;
/// A mapping of file names to their section maps.
type FileMap = HashMap<String, SectionMap>;
/// A mapping of language codes to file maps.
type LangMap = HashMap<String, FileMap>;

/// Contains all translations loaded from filesystem or bundled data.
/// 
/// Organized as: `languages -> files -> keys -> values`
#[derive(Debug, Deserialize)]
pub struct Translations {
    /// Map of language codes to their translation data
    pub langs: LangMap,
}

/// Main resource for accessing translations in Bevy systems.
/// 
/// Provides methods to load translation files, get translated text,
/// and manage current language settings.
/// 
/// # Example
/// 
/// ```rust
/// use bevy::prelude::*;
/// use bevy_intl::I18n;
/// 
/// fn my_system(i18n: Res<I18n>) {
///     let translations = i18n.translation("ui");
///     let text = translations.t("welcome_message");
///     println!("{}", text);
/// }
/// ```
#[derive(Resource)]
pub struct I18n {
    /// All loaded translations
    translations: Translations,
    /// Currently active language
    current_lang: String,
    /// List of available languages
    locale_folders_list: Vec<String>,
    /// Fallback language when translation is missing
    fallback_lang: String,
    /// Per-locale CLDR plural rules. Locales for which no rules could be
    /// resolved (custom dialects, unknown codes) are absent from this map and
    /// fall back to anglo-centric defaults inside `t_with_plural`.
    plural_rules: HashMap<String, PluralRules>,
}

impl FromWorld for I18n {
    fn from_world(world: &mut World) -> Self {
        let config = world.get_resource::<I18nConfig>().cloned().unwrap_or_default();

        let (translations, locale_folders_list) = if config.use_bundled_translations {
            load_bundled_translations()
        } else {
            load_filesystem_translations(&config.messages_folder)
        };

        if config.warn_unknown_locales {
            for locale in &locale_folders_list {
                if !locale_exists_as_international_standard(locale) {
                    warn!(
                        "Locale folder '{}' is not a recognized ISO/CLDR locale code",
                        locale
                    );
                }
            }
        }

        if !locale_folders_list.contains(&config.default_lang) {
            warn!(
                "Default language '{}' not found in loaded translations (available: {:?})",
                config.default_lang, locale_folders_list
            );
        }
        if !locale_folders_list.contains(&config.fallback_lang) {
            warn!(
                "Fallback language '{}' not found in loaded translations (available: {:?})",
                config.fallback_lang, locale_folders_list
            );
        }

        let plural_rules = build_plural_rules(&locale_folders_list);

        Self {
            current_lang: config.default_lang,
            fallback_lang: config.fallback_lang,
            translations,
            locale_folders_list,
            plural_rules,
        }
    }
}

fn build_plural_rules(locales: &[String]) -> HashMap<String, PluralRules> {
    let mut map = HashMap::new();
    for lang in locales {
        match lang.parse::<LanguageIdentifier>() {
            Ok(langid) => match PluralRules::create(langid, PluralRuleType::CARDINAL) {
                Ok(rules) => {
                    map.insert(lang.clone(), rules);
                }
                Err(e) => warn!("no CLDR plural rules for '{}': {}", lang, e),
            },
            Err(e) => warn!("could not parse '{}' as a language identifier: {}", lang, e),
        }
    }
    map
}

fn cldr_category_to_str(cat: PluralCategory) -> &'static str {
    match cat {
        PluralCategory::ZERO => "zero",
        PluralCategory::ONE => "one",
        PluralCategory::TWO => "two",
        PluralCategory::FEW => "few",
        PluralCategory::MANY => "many",
        PluralCategory::OTHER => "other",
    }
}

// ---------- Loaders ----------

// Loading from filesystem (dev/desktop mode)
#[cfg(not(target_arch = "wasm32"))]
fn load_filesystem_translations(messages_folder: &str) -> (Translations, Vec<String>) {
    match load_translation_from_fs(messages_folder) {
        Ok(langs) => build_translations(langs),
        Err(e) => {
            warn!("Failed to load translations from '{}': {}", messages_folder, e);
            create_error_translations()
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_filesystem_translations(_messages_folder: &str) -> (Translations, Vec<String>) {
    // Filesystem loading is unavailable on WASM. Returning error_translations
    // here (rather than calling load_bundled_translations) avoids the infinite
    // recursion that would occur if bundled data is also empty.
    warn!("Filesystem loading not available on WASM");
    create_error_translations()
}

// Loading from bundled translations (bundled at build time)
fn load_bundled_translations() -> (Translations, Vec<String>) {
    match load_bundled_data() {
        Ok(langs) => {
            if langs.is_empty() {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    warn!("Bundled translations empty, falling back to filesystem");
                    return load_filesystem_translations("messages");
                }
                #[cfg(target_arch = "wasm32")]
                {
                    warn!("Bundled translations empty on WASM (no fallback available)");
                    return create_error_translations();
                }
            }
            build_translations(langs)
        }
        Err(e) => {
            warn!("Failed to load bundled translations: {}", e);
            create_error_translations()
        }
    }
}

// Shared helper to convert a LangMap into the Translations struct + sorted locale list
fn build_translations(langs: LangMap) -> (Translations, Vec<String>) {
    let mut locale_list: Vec<String> = langs.keys().cloned().collect();
    locale_list.sort();
    (Translations { langs }, locale_list)
}

// Load bundled data (generated by build.rs)
fn load_bundled_data() -> Result<LangMap, Box<dyn std::error::Error>> {
    const BUNDLED_TRANSLATIONS: &str = include_str!(
        concat!(env!("OUT_DIR"), "/all_translations.json")
    );
    
    // Check if bundled translations are empty (happens when bevy-intl is built standalone)
    let value: Value = serde_json::from_str(BUNDLED_TRANSLATIONS)?;
    if !matches!(value.as_object(), Some(obj) if !obj.is_empty()) {
        // Return empty translation map - will fall back to filesystem loading
        return Ok(HashMap::new());
    }
    
    parse_translation_value(value)
}

// Parse a JSON Value to LangMap
fn parse_translation_value(value: Value) -> Result<LangMap, Box<dyn std::error::Error>> {
    let mut lang_map = HashMap::new();

    if let Some(langs_obj) = value.as_object() {
        for (lang_code, files_value) in langs_obj {
            let mut file_map = HashMap::new();

            if let Some(files_obj) = files_value.as_object() {
                for (file_name, sections_value) in files_obj {
                    let mut section_map = HashMap::new();

                    if let Some(sections_obj) = sections_value.as_object() {
                        for (key, val) in sections_obj {
                            if let Some(section_value) = parse_section_value(val) {
                                section_map.insert(key.clone(), section_value);
                            }
                        }
                    }
                    file_map.insert(file_name.clone(), section_map);
                }
            }
            lang_map.insert(lang_code.clone(), file_map);
        }
    }

    Ok(lang_map)
}

// Filesystem version
#[cfg(not(target_arch = "wasm32"))]
fn load_translation_from_fs(messages_folder: &str) -> std::io::Result<LangMap> {
    use std::fs;
    use std::path::Path;

    let message_dir = Path::new(messages_folder);

    if !message_dir.exists() {
        return Err(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} folder not found", messages_folder)
            )
        );
    }

    let mut lang_map = HashMap::new();

    for folder_entry in fs::read_dir(message_dir)? {
        let folder = folder_entry?;
        let lang_code = folder.file_name().to_string_lossy().to_string();
        let mut file_map = HashMap::new();

        for file_entry in fs::read_dir(folder.path())? {
            let file = file_entry?;
            let path = file.path();

            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
                let file_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let content = fs::read_to_string(&path)?;
                let json: Value = serde_json
                    ::from_str(&content)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

                let mut section_map = HashMap::new();

                if let Some(obj) = json.as_object() {
                    for (key, value) in obj {
                        if let Some(section_value) = parse_section_value(value) {
                            section_map.insert(key.clone(), section_value);
                        }
                    }
                }

                file_map.insert(file_name, section_map);
            }
        }

        lang_map.insert(lang_code, file_map);
    }

    Ok(lang_map)
}

/// Convert a `serde_json::Value` into a [`SectionValue`], picking the best
/// variant based on shape:
///
/// - String → [`SectionValue::Text`]
/// - Object whose values are *all* objects → [`SectionValue::Nested`] (gender × plural)
/// - Otherwise object → [`SectionValue::Map`] (single-axis: plural OR gender)
/// - Anything else (number, array, null) → `None` (entry is skipped)
fn parse_section_value(val: &Value) -> Option<SectionValue> {
    if let Some(text) = val.as_str() {
        return Some(SectionValue::Text(text.to_string()));
    }
    let obj = val.as_object()?;

    let has_only_object_values = !obj.is_empty()
        && obj.values().all(|v| v.is_object());

    if has_only_object_values {
        let mut nested = HashMap::new();
        for (k, v) in obj {
            if let Some(inner_obj) = v.as_object() {
                let mut inner = HashMap::new();
                for (ik, iv) in inner_obj {
                    if let Some(s) = iv.as_str() {
                        inner.insert(ik.clone(), s.to_string());
                    }
                }
                nested.insert(k.clone(), inner);
            }
        }
        return Some(SectionValue::Nested(nested));
    }

    let mut map = HashMap::new();
    for (k, v) in obj {
        if let Some(s) = v.as_str() {
            map.insert(k.clone(), s.to_string());
        }
    }
    Some(SectionValue::Map(map))
}

// Default error translations
fn create_error_translations() -> (Translations, Vec<String>) {
    let mut section_map = HashMap::new();
    section_map.insert("error".to_string(), SectionValue::Text("Translation Error".to_string()));

    let mut file_map = HashMap::new();
    file_map.insert("error".to_string(), section_map);

    let mut lang_map = HashMap::new();
    lang_map.insert("en".to_string(), file_map);

    (Translations { langs: lang_map }, vec!["en".to_string()])
}

// ---------- API ----------

/// Errors returned by fallible operations on [`I18n`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum I18nError {
    /// The requested locale was not found in the loaded translations.
    LocaleNotFound(String),
}

impl std::fmt::Display for I18nError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            I18nError::LocaleNotFound(loc) => {
                write!(f, "locale '{}' not found in loaded translations", loc)
            }
        }
    }
}

impl std::error::Error for I18nError {}

/// Extension trait for `App` to set languages at startup, before `run()`.
///
/// `App` is not a Bevy `Resource`, so these methods are intended to be called
/// during plugin setup (build-time configuration), not from inside a system.
/// For runtime changes, use [`I18n::set_lang`] / [`I18n::try_set_lang`].
///
/// # Example
///
/// ```rust,no_run
/// use bevy::prelude::*;
/// use bevy_intl::{I18nPlugin, LanguageAppExt};
///
/// App::new()
///     .add_plugins(I18nPlugin::default())
///     .set_lang_i18n("fr")
///     .set_fallback_lang("en")
///     .run();
/// ```
pub trait LanguageAppExt {
    /// Sets the current language for translations. Logs a warning if the locale
    /// is not available in the loaded translations. Returns `&mut Self` so it
    /// chains with the rest of the `App` builder.
    fn set_lang_i18n(&mut self, locale: &str) -> &mut Self;
    /// Sets the fallback language for translations. Logs a warning if the locale
    /// is not available in the loaded translations.
    fn set_fallback_lang(&mut self, locale: &str) -> &mut Self;
}

impl LanguageAppExt for App {
    fn set_lang_i18n(&mut self, locale: &str) -> &mut Self {
        if let Some(mut i18n) = self.world_mut().get_resource_mut::<I18n>() {
            i18n.set_lang(locale);
        }
        self
    }

    fn set_fallback_lang(&mut self, locale: &str) -> &mut Self {
        if let Some(mut i18n) = self.world_mut().get_resource_mut::<I18n>() {
            i18n.set_fallback_lang(locale);
        }
        self
    }
}

// ---------- Translation Handling ----------

/// Represents translations for a single file.
/// 
/// Provides methods to access translated text with support for
/// placeholders, plurals, and gendered translations.
/// 
/// # Example
/// 
/// ```rust
/// use bevy::prelude::*;
/// use bevy_intl::I18n;
/// 
/// fn display_text(i18n: Res<I18n>) {
///     let t = i18n.translation("ui");
///     
///     // Simple translation
///     let greeting = t.t("hello");
///     
///     // With placeholder
///     let welcome = t.t_with_arg("welcome", &[&"John"]);
///     
///     // Plural form
///     let items = t.t_with_plural("item_count", 5);
///     
///     // Gendered translation
///     let title = t.t_with_gender("title", "male");
/// }
/// ```
pub struct I18nPartial<'a> {
    /// Translations for the current language (borrowed from `I18n`)
    file_translations: &'a SectionMap,
    /// Fallback translations when current language is missing a key (borrowed from `I18n`)
    fallback_translation: &'a SectionMap,
    /// CLDR plural rules for the current language (`None` for unknown locales)
    plural_rules: Option<&'a PluralRules>,
}

/// An empty section map used as a sentinel when a requested translation file
/// is missing — keeps `I18nPartial` zero-copy without needing a `Cow`.
static EMPTY_SECTION_MAP: LazyLock<SectionMap> = LazyLock::new(HashMap::new);

impl I18n {
    /// Loads translations for a specific file.
    ///
    /// Returns an `I18nPartial` that borrows from `self` and provides access
    /// to all translation methods for that file.
    ///
    /// # Arguments
    ///
    /// * `translation_file` - Name of the translation file (without .json extension)
    ///
    /// # Example
    ///
    /// ```rust
    /// use bevy::prelude::*;
    /// use bevy_intl::I18n;
    ///
    /// fn my_system(i18n: Res<I18n>) {
    ///     let ui_translations = i18n.translation("ui");
    ///     let menu_translations = i18n.translation("menu");
    /// }
    /// ```
    pub fn translation<'a>(&'a self, translation_file: &str) -> I18nPartial<'a> {
        let file_translations = self.translations.langs
            .get(&self.current_lang)
            .and_then(|lang| lang.get(translation_file))
            .unwrap_or(&EMPTY_SECTION_MAP);

        let fallback_translation = self.translations.langs
            .get(&self.fallback_lang)
            .and_then(|lang| lang.get(translation_file))
            .unwrap_or(&EMPTY_SECTION_MAP);

        let plural_rules = self.plural_rules.get(&self.current_lang);

        I18nPartial { file_translations, fallback_translation, plural_rules }
    }

    /// Sets the current language. Logs a warning when the locale is unknown.
    ///
    /// For programmatic error handling, use [`try_set_lang`](Self::try_set_lang).
    ///
    /// # Example
    ///
    /// ```rust
    /// # use bevy::prelude::*; use bevy_intl::I18n;
    /// fn change_language(mut i18n: ResMut<I18n>) {
    ///     i18n.set_lang("fr");
    /// }
    /// ```
    pub fn set_lang(&mut self, locale: &str) {
        if let Err(e) = self.try_set_lang(locale) {
            warn!("{}", e);
        }
    }

    /// Sets the current language, returning [`I18nError::LocaleNotFound`] if
    /// the locale is not part of the loaded translations. The current language
    /// is left unchanged on error.
    pub fn try_set_lang(&mut self, locale: &str) -> Result<(), I18nError> {
        if !self.locale_folders_list.iter().any(|l| l == locale) {
            return Err(I18nError::LocaleNotFound(locale.to_string()));
        }
        self.current_lang = locale.to_string();
        Ok(())
    }

    /// Sets the fallback language. Logs a warning when the locale is unknown.
    pub fn set_fallback_lang(&mut self, locale: &str) {
        if let Err(e) = self.try_set_fallback_lang(locale) {
            warn!("{}", e);
        }
    }

    /// Sets the fallback language, returning [`I18nError::LocaleNotFound`] if
    /// the locale is not part of the loaded translations.
    pub fn try_set_fallback_lang(&mut self, locale: &str) -> Result<(), I18nError> {
        if !self.locale_folders_list.iter().any(|l| l == locale) {
            return Err(I18nError::LocaleNotFound(locale.to_string()));
        }
        self.fallback_lang = locale.to_string();
        Ok(())
    }

    /// Gets the current fallback language code.
    pub fn get_fallback_lang(&self) -> &str {
        &self.fallback_lang
    }

    /// Gets the current language code.
    /// 
    /// # Returns
    /// 
    /// The current language code as a string slice.
    /// 
    /// # Example
    /// 
    /// ```rust
    /// use bevy::prelude::*;
    /// use bevy_intl::I18n;
    /// 
    /// fn show_current_language(i18n: Res<I18n>) {
    ///     println!("Current language: {}", i18n.get_lang());
    /// }
    /// ```
    pub fn get_lang(&self) -> &str {
        &self.current_lang
    }

    /// Gets a list of all available languages.
    /// 
    /// # Returns
    /// 
    /// A slice of available language codes.
    /// 
    /// # Example
    /// 
    /// ```rust
    /// use bevy::prelude::*;
    /// use bevy_intl::I18n;
    /// 
    /// fn list_languages(i18n: Res<I18n>) {
    ///     for lang in i18n.available_languages() {
    ///         println!("Available: {}", lang);
    ///     }
    /// }
    /// ```
    pub fn available_languages(&self) -> &[String] {
        &self.locale_folders_list
    }
}

// ---------- Text helpers ----------
static ARG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{(\w+)\}\}").unwrap());

impl I18nPartial<'_> {
    /// Gets a translated string for the given key.
    /// 
    /// Falls back to the fallback language if the key is not found
    /// in the current language.
    /// 
    /// # Arguments
    /// 
    /// * `key` - Translation key to look up
    /// 
    /// # Returns
    /// 
    /// The translated string, or "Missing translation" if not found.
    /// 
    /// # Example
    /// 
    /// ```rust
    /// let text = i18n.translation("ui").t("hello");
    /// ```
    pub fn t(&self, key: &str) -> String {
        match self.get_text_value(key) {
            Some(s) => s,
            None => {
                warn!("translation key '{}' not found (no fallback either)", key);
                "Missing translation".to_string()
            }
        }
    }

    /// Gets a translated string with **named** placeholder replacement.
    ///
    /// Replaces `{{name}}` placeholders by matching their name to the keys in
    /// `args`. Unmatched placeholders are kept literally and a warning is emitted.
    ///
    /// # Arguments
    ///
    /// * `key` - Translation key to look up
    /// * `args` - Slice of `(name, value)` pairs to substitute into placeholders
    ///
    /// # Example
    ///
    /// ```rust
    /// // JSON: "welcome": "Hello {{name}}, you have {{count}} messages"
    /// // Either with the macro:
    /// let text = i18n.translation("ui").t_with_args("welcome", i18n_args!{ name = "John", count = 5 });
    /// // Or as explicit tuples:
    /// let text = i18n.translation("ui").t_with_args("welcome", &[("name", &"John"), ("count", &5)]);
    /// // Result: "Hello John, you have 5 messages"
    /// ```
    pub fn t_with_args(&self, key: &str, args: &[(&str, &dyn ToString)]) -> String {
        let template = self.t(key);
        replace_named_placeholders(&template, args)
    }

    /// Gets a translated string with positional placeholder replacement.
    ///
    /// **Deprecated since 0.3.0** — placeholder names in the JSON are ignored
    /// and arguments are consumed in the order they appear in the template.
    /// Use [`t_with_args`](Self::t_with_args) for proper named substitution.
    ///
    /// # Arguments
    ///
    /// * `key` - Translation key to look up
    /// * `args` - Values to replace placeholders with, by order of appearance
    #[deprecated(
        since = "0.3.0",
        note = "use `t_with_args` with named tuples (or the `i18n_args!` macro) for proper named placeholder substitution"
    )]
    pub fn t_with_arg(&self, key: &str, args: &[&dyn ToString]) -> String {
        let template = self.t(key);
        replace_positional_placeholders(&template, args)
    }

    /// Gets a pluralized translation based on count.
    /// 
    /// Uses advanced plural rules with fallback priority:
    /// 1. Exact count ("0", "1", "2", etc.)
    /// 2. ICU categories ("zero", "one", "two", "few", "many")
    /// 3. Basic fallback ("one" vs "other")
    /// 
    /// # Arguments
    /// 
    /// * `key` - Translation key to look up
    /// * `count` - Number to determine plural form
    /// 
    /// # Returns
    /// 
    /// The translated string with count placeholder replaced.
    /// 
    /// # Example
    /// 
    /// ```rust
    /// // JSON: "items": { "one": "One item", "many": "{{count}} items" }
    /// let text = i18n.translation("ui").t_with_plural("items", 5);
    /// // Result: "5 items"
    /// ```
    pub fn t_with_plural(&self, key: &str, count: usize) -> String {
        // 1. Try exact count first (e.g., "0", "1", "2"...) — most specific.
        let count_str = count.to_string();
        if let Some(template) = self.get_nested_value(key, &count_str) {
            return replace_named_placeholders(&template, &[("count", &count)]);
        }

        // 2. Try the plural category for the active language. The category is
        //    resolved through CLDR rules when an `I18n` was provided to this
        //    `I18nPartial` (default path); otherwise the basic anglo-centric
        //    fallback below applies.
        if let Some(category) = self.plural_category(count) {
            if let Some(template) = self.get_nested_value(key, category) {
                return replace_named_placeholders(&template, &[("count", &count)]);
            }
        }

        // 3. Fallback to basic English rules ("one" / "other").
        let basic_key = if count == 1 { "one" } else { "other" };
        if let Some(template) = self.get_nested_value(key, basic_key) {
            return replace_named_placeholders(&template, &[("count", &count)]);
        }

        // 4. Last resort: "many".
        if let Some(template) = self.get_nested_value(key, "many") {
            return replace_named_placeholders(&template, &[("count", &count)]);
        }

        warn!("plural translation '{}' not found for count {}", key, count);
        "Missing plural translation".to_string()
    }

    /// Resolve a plural category for `count` in the active language using
    /// CLDR rules when available, falling back to anglo-centric defaults.
    fn plural_category(&self, count: usize) -> Option<&'static str> {
        if let Some(rules) = self.plural_rules {
            match rules.select(count) {
                Ok(cat) => return Some(cldr_category_to_str(cat)),
                Err(e) => warn!("CLDR plural rule selection failed: {}", e),
            }
        }
        Some(basic_plural_category(count))
    }

    /// Gets a translation that varies by **both** gender and plural count.
    ///
    /// The JSON layout is `{ key: { gender: { plural_category: "..." } } }`,
    /// e.g.:
    ///
    /// ```json
    /// "guests": {
    ///     "male":   { "one": "{{count}} guest (M)", "other": "{{count}} guests (M)" },
    ///     "female": { "one": "{{count}} guest (F)", "other": "{{count}} guests (F)" }
    /// }
    /// ```
    ///
    /// Plural-category resolution uses the same CLDR rules as
    /// [`t_with_plural`](Self::t_with_plural), with exact-count keys taking
    /// priority.
    pub fn t_with_gender_and_plural(&self, key: &str, gender: &str, count: usize) -> String {
        let count_str = count.to_string();
        if let Some(template) = self.get_gender_plural_value(key, gender, &count_str) {
            return replace_named_placeholders(&template, &[("count", &count)]);
        }
        if let Some(category) = self.plural_category(count) {
            if let Some(template) = self.get_gender_plural_value(key, gender, category) {
                return replace_named_placeholders(&template, &[("count", &count)]);
            }
        }
        let basic_key = if count == 1 { "one" } else { "other" };
        if let Some(template) = self.get_gender_plural_value(key, gender, basic_key) {
            return replace_named_placeholders(&template, &[("count", &count)]);
        }

        warn!(
            "gender+plural translation '{}' missing for gender '{}' count {}",
            key, gender, count
        );
        "Missing gender+plural translation".to_string()
    }

    fn get_gender_plural_value(
        &self,
        key: &str,
        gender: &str,
        plural_key: &str,
    ) -> Option<String> {
        let pick = |sm: &SectionMap| -> Option<String> {
            match sm.get(key)? {
                SectionValue::Nested(map) => map.get(gender)?.get(plural_key).cloned(),
                _ => None,
            }
        };
        pick(self.file_translations).or_else(|| pick(self.fallback_translation))
    }

    /// Gets a gendered translation.
    /// 
    /// # Arguments
    /// 
    /// * `key` - Translation key to look up
    /// * `gender` - Gender key (e.g., "male", "female", "neutral")
    /// 
    /// # Returns
    /// 
    /// The translated string for the specified gender.
    /// 
    /// # Example
    /// 
    /// ```rust
    /// // JSON: "title": { "male": "Mr.", "female": "Ms." }
    /// let text = i18n.translation("ui").t_with_gender("title", "female");
    /// // Result: "Ms."
    /// ```
    pub fn t_with_gender(&self, key: &str, gender: &str) -> String {
        self.get_nested_value(key, gender).unwrap_or_else(||
            "Missing gender translation".to_string()
        )
    }

    /// Gets a gendered translation with **named** placeholder replacement.
    ///
    /// Combines gender selection and named argument substitution.
    ///
    /// # Example
    ///
    /// ```rust
    /// // JSON: "greeting": { "male": "Hello Mr. {{name}}", "female": "Hello Ms. {{name}}" }
    /// let text = i18n.translation("ui").t_with_gender_and_args(
    ///     "greeting", "male", i18n_args!{ name = "Smith" }
    /// );
    /// // Result: "Hello Mr. Smith"
    /// ```
    pub fn t_with_gender_and_args(
        &self,
        key: &str,
        gender: &str,
        args: &[(&str, &dyn ToString)],
    ) -> String {
        let template = self.t_with_gender(key, gender);
        replace_named_placeholders(&template, args)
    }

    /// Gets a gendered translation with positional placeholder replacement.
    ///
    /// **Deprecated since 0.3.0** — use [`t_with_gender_and_args`](Self::t_with_gender_and_args).
    #[deprecated(
        since = "0.3.0",
        note = "use `t_with_gender_and_args` with named tuples (or the `i18n_args!` macro)"
    )]
    pub fn t_with_gender_and_arg(
        &self,
        key: &str,
        gender: &str,
        args: &[&dyn ToString],
    ) -> String {
        let template = self.t_with_gender(key, gender);
        replace_positional_placeholders(&template, args)
    }

    // Private utility methods
    fn get_text_value(&self, key: &str) -> Option<String> {
        self.file_translations
            .get(key)
            .and_then(|v| if let SectionValue::Text(s) = v { Some(s.clone()) } else { None })
            .or_else(|| {
                self.fallback_translation
                    .get(key)
                    .and_then(|v| (
                        if let SectionValue::Text(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    ))
            })
    }

    fn get_nested_value(&self, key: &str, nested_key: &str) -> Option<String> {
        self.file_translations
            .get(key)
            .and_then(|v| (
                if let SectionValue::Map(m) = v {
                    m.get(nested_key).cloned()
                } else {
                    None
                }
            ))
            .or_else(|| {
                self.fallback_translation
                    .get(key)
                    .and_then(|v| (
                        if let SectionValue::Map(m) = v {
                            m.get(nested_key).cloned()
                        } else {
                            None
                        }
                    ))
            })
    }

}

// ---------- Placeholder helpers ----------

/// Replace `{{name}}` placeholders by looking up the matching `(name, value)`
/// pair in `args`. Unknown names are kept literally and a warning is logged.
fn replace_named_placeholders(template: &str, args: &[(&str, &dyn ToString)]) -> String {
    ARG_RE
        .replace_all(template, |caps: &regex::Captures<'_>| {
            let name = &caps[1];
            match args.iter().find(|(k, _)| *k == name) {
                Some((_, v)) => v.to_string(),
                None => {
                    warn!("missing value for placeholder '{{{{{}}}}}'", name);
                    caps[0].to_string()
                }
            }
        })
        .into_owned()
}

/// Replace `{{...}}` placeholders **by order of appearance** (positional).
/// Used by the deprecated `t_with_arg` / `t_with_gender_and_arg` API to keep
/// existing callers working until they migrate to the named API.
fn replace_positional_placeholders(template: &str, args: &[&dyn ToString]) -> String {
    let counter = std::cell::Cell::new(0usize);
    ARG_RE
        .replace_all(template, |caps: &regex::Captures<'_>| {
            let i = counter.get();
            counter.set(i + 1);
            match args.get(i) {
                Some(v) => v.to_string(),
                None => caps[0].to_string(),
            }
        })
        .into_owned()
}

/// Anglo-centric plural category fallback used when no per-language CLDR
/// rules are available. The CLDR-correct path is registered at runtime via
/// [`I18n`]'s plural rules; this function only acts as a last resort.
fn basic_plural_category(count: usize) -> &'static str {
    match count {
        0 => "zero",
        1 => "one",
        2 => "two",
        3..=10 => "few",
        _ => "many",
    }
}

// ---------- Utils ----------

/// Checks if a locale string exists as an international standard.
///
/// Uses the built-in LOCALES list to validate locale codes against
/// international standards (ISO 639-1, ISO 3166-1, etc.).
fn locale_exists_as_international_standard(locale: &str) -> bool {
    LOCALES.binary_search(&locale).is_ok()
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_section(pairs: &[(&str, SectionValue)]) -> SectionMap {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn make_i18n(current: &str, fallback: &str, langs: LangMap) -> I18n {
        let mut locale_folders_list: Vec<String> = langs.keys().cloned().collect();
        locale_folders_list.sort();
        let plural_rules = build_plural_rules(&locale_folders_list);
        I18n {
            current_lang: current.to_string(),
            fallback_lang: fallback.to_string(),
            translations: Translations { langs },
            locale_folders_list,
            plural_rules,
        }
    }

    fn single_lang(lang: &str, file: &str, sections: SectionMap) -> LangMap {
        let mut file_map: FileMap = HashMap::new();
        file_map.insert(file.to_string(), sections);
        let mut lang_map = HashMap::new();
        lang_map.insert(lang.to_string(), file_map);
        lang_map
    }

    // --- Placeholder helpers ---

    #[test]
    fn replace_named_basic() {
        let out = replace_named_placeholders("Hi {{name}}", &[("name", &"John")]);
        assert_eq!(out, "Hi John");
    }

    #[test]
    fn replace_named_two_args_any_order() {
        // The whole point of named placeholders: insertion order in the args
        // slice does not matter — we look up by name.
        let out = replace_named_placeholders(
            "{{name}} has {{count}} apples",
            &[("count", &5), ("name", &"John")],
        );
        assert_eq!(out, "John has 5 apples");
    }

    #[test]
    fn replace_named_missing_arg_keeps_literal() {
        let out = replace_named_placeholders("Hi {{name}}", &[]);
        assert_eq!(out, "Hi {{name}}");
    }

    #[test]
    fn replace_positional_ordered() {
        let one = 1i32;
        let two = 2i32;
        let out =
            replace_positional_placeholders("{{a}} and {{b}}", &[&one as &dyn ToString, &two]);
        assert_eq!(out, "1 and 2");
    }

    #[test]
    fn replace_positional_too_few_args_keeps_remaining() {
        let one = 1i32;
        let out = replace_positional_placeholders("{{a}} and {{b}}", &[&one as &dyn ToString]);
        assert_eq!(out, "1 and {{b}}");
    }

    // --- Macro ---

    #[test]
    fn i18n_args_macro_expansion() {
        let args = i18n_args!{ name = "John", count = 5 };
        // Reconstruct the strings to avoid relying on `dyn ToString` PartialEq.
        let collected: Vec<(&str, String)> =
            args.iter().map(|(k, v)| (*k, v.to_string())).collect();
        assert_eq!(collected, vec![("name", "John".into()), ("count", "5".into())]);
    }

    #[test]
    fn i18n_args_macro_empty() {
        let args = i18n_args!{};
        assert!(args.is_empty());
    }

    // --- Locale ISO check ---

    #[test]
    fn locale_iso_check() {
        assert!(locale_exists_as_international_standard("fr"));
        assert!(locale_exists_as_international_standard("fr-BE"));
        assert!(!locale_exists_as_international_standard("klingon"));
    }

    // --- parse_section_value ---

    #[test]
    fn parse_section_value_text() {
        let v: Value = serde_json::from_str(r#""hello""#).unwrap();
        match parse_section_value(&v) {
            Some(SectionValue::Text(s)) => assert_eq!(s, "hello"),
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn parse_section_value_map() {
        let v: Value = serde_json::from_str(r#"{"one":"a","other":"b"}"#).unwrap();
        match parse_section_value(&v) {
            Some(SectionValue::Map(m)) => {
                assert_eq!(m.get("one").map(String::as_str), Some("a"));
                assert_eq!(m.get("other").map(String::as_str), Some("b"));
            }
            other => panic!("expected Map, got {:?}", other),
        }
    }

    #[test]
    fn parse_section_value_nested() {
        let v: Value =
            serde_json::from_str(r#"{"male":{"one":"a"},"female":{"one":"b"}}"#).unwrap();
        match parse_section_value(&v) {
            Some(SectionValue::Nested(m)) => {
                assert_eq!(m.get("male").and_then(|i| i.get("one")).map(String::as_str), Some("a"));
                assert_eq!(m.get("female").and_then(|i| i.get("one")).map(String::as_str), Some("b"));
            }
            other => panic!("expected Nested, got {:?}", other),
        }
    }

    #[test]
    fn parse_section_value_invalid_returns_none_for_array() {
        let v: Value = serde_json::from_str("[1,2,3]").unwrap();
        assert!(parse_section_value(&v).is_none());
    }

    // --- Plural categories ---

    #[test]
    fn basic_plural_category_buckets() {
        assert_eq!(basic_plural_category(0), "zero");
        assert_eq!(basic_plural_category(1), "one");
        assert_eq!(basic_plural_category(2), "two");
        assert_eq!(basic_plural_category(5), "few");
        assert_eq!(basic_plural_category(10), "few");
        assert_eq!(basic_plural_category(11), "many");
    }

    #[test]
    fn cldr_polish_categories() {
        // Polish: 1 → one, 2/3/4 → few, 5..=21 → many, 22..=24 → few, …
        let langid: LanguageIdentifier = "pl".parse().unwrap();
        let pr = PluralRules::create(langid, PluralRuleType::CARDINAL).unwrap();
        assert_eq!(cldr_category_to_str(pr.select(1usize).unwrap()), "one");
        assert_eq!(cldr_category_to_str(pr.select(2usize).unwrap()), "few");
        assert_eq!(cldr_category_to_str(pr.select(5usize).unwrap()), "many");
    }

    #[test]
    fn cldr_russian_categories() {
        let langid: LanguageIdentifier = "ru".parse().unwrap();
        let pr = PluralRules::create(langid, PluralRuleType::CARDINAL).unwrap();
        assert_eq!(cldr_category_to_str(pr.select(1usize).unwrap()), "one");
        assert_eq!(cldr_category_to_str(pr.select(2usize).unwrap()), "few");
        assert_eq!(cldr_category_to_str(pr.select(5usize).unwrap()), "many");
        assert_eq!(cldr_category_to_str(pr.select(11usize).unwrap()), "many");
    }

    #[test]
    fn cldr_arabic_categories() {
        let langid: LanguageIdentifier = "ar".parse().unwrap();
        let pr = PluralRules::create(langid, PluralRuleType::CARDINAL).unwrap();
        assert_eq!(cldr_category_to_str(pr.select(0usize).unwrap()), "zero");
        assert_eq!(cldr_category_to_str(pr.select(1usize).unwrap()), "one");
        assert_eq!(cldr_category_to_str(pr.select(2usize).unwrap()), "two");
    }

    // --- I18nPartial end-to-end ---

    #[test]
    fn t_returns_value() {
        let i18n = make_i18n(
            "en",
            "en",
            single_lang(
                "en",
                "ui",
                make_section(&[("greeting", SectionValue::Text("Hello".into()))]),
            ),
        );
        assert_eq!(i18n.translation("ui").t("greeting"), "Hello");
    }

    #[test]
    fn t_with_args_named() {
        let mut langs = LangMap::new();
        let mut files = FileMap::new();
        files.insert(
            "ui".into(),
            make_section(&[(
                "welcome",
                SectionValue::Text("Hi {{name}}, you have {{count}} messages".into()),
            )]),
        );
        langs.insert("en".into(), files);
        let i18n = make_i18n("en", "en", langs);

        let t = i18n.translation("ui");
        let out = t.t_with_args(
            "welcome",
            &[("name", &"John"), ("count", &5)],
        );
        assert_eq!(out, "Hi John, you have 5 messages");
    }

    #[test]
    fn t_with_plural_polish() {
        let mut sections = make_section(&[(
            "apples",
            SectionValue::Map(
                [
                    ("one".into(), "{{count}} jabłko".into()),
                    ("few".into(), "{{count}} jabłka".into()),
                    ("many".into(), "{{count}} jabłek".into()),
                ]
                .into_iter()
                .collect(),
            ),
        )]);
        // Add an exact-count override to verify priority.
        sections.insert(
            "free".into(),
            SectionValue::Map(
                [("0".into(), "Brak".into()), ("other".into(), "{{count}} szt".into())]
                    .into_iter()
                    .collect(),
            ),
        );
        let mut files = FileMap::new();
        files.insert("ui".into(), sections);
        let mut langs = LangMap::new();
        langs.insert("pl".into(), files);
        let i18n = make_i18n("pl", "pl", langs);
        let t = i18n.translation("ui");

        // CLDR Polish: 1 → one, 2 → few, 5 → many.
        assert_eq!(t.t_with_plural("apples", 1), "1 jabłko");
        assert_eq!(t.t_with_plural("apples", 2), "2 jabłka");
        assert_eq!(t.t_with_plural("apples", 5), "5 jabłek");

        // Exact-count beats CLDR.
        assert_eq!(t.t_with_plural("free", 0), "Brak");
    }

    #[test]
    fn t_with_gender_and_plural() {
        let mut male = HashMap::new();
        male.insert("one".into(), "{{count}} guest (M)".into());
        male.insert("other".into(), "{{count}} guests (M)".into());
        let mut female = HashMap::new();
        female.insert("one".into(), "{{count}} guest (F)".into());
        female.insert("other".into(), "{{count}} guests (F)".into());
        let mut nested = HashMap::new();
        nested.insert("male".into(), male);
        nested.insert("female".into(), female);

        let sections = make_section(&[("guests", SectionValue::Nested(nested))]);
        let mut files = FileMap::new();
        files.insert("ui".into(), sections);
        let mut langs = LangMap::new();
        langs.insert("en".into(), files);
        let i18n = make_i18n("en", "en", langs);
        let t = i18n.translation("ui");

        assert_eq!(t.t_with_gender_and_plural("guests", "male", 1), "1 guest (M)");
        assert_eq!(
            t.t_with_gender_and_plural("guests", "female", 3),
            "3 guests (F)"
        );
    }

    #[test]
    fn fallback_used_when_key_missing() {
        let mut en_files = FileMap::new();
        en_files.insert(
            "ui".into(),
            make_section(&[("greet", SectionValue::Text("Hello".into()))]),
        );
        let mut fr_files = FileMap::new();
        // fr has the file but no `greet`, so we fall back to en.
        fr_files.insert("ui".into(), make_section(&[]));

        let mut langs = LangMap::new();
        langs.insert("en".into(), en_files);
        langs.insert("fr".into(), fr_files);
        let i18n = make_i18n("fr", "en", langs);

        assert_eq!(i18n.translation("ui").t("greet"), "Hello");
    }

    #[test]
    fn try_set_lang_unknown_returns_err() {
        let mut i18n = make_i18n(
            "en",
            "en",
            single_lang("en", "ui", make_section(&[])),
        );
        assert_eq!(
            i18n.try_set_lang("xx"),
            Err(I18nError::LocaleNotFound("xx".into()))
        );
        // Current language unchanged.
        assert_eq!(i18n.get_lang(), "en");
    }

    #[test]
    fn try_set_lang_known_succeeds() {
        let mut langs = LangMap::new();
        langs.insert("en".into(), FileMap::new());
        langs.insert("fr".into(), FileMap::new());
        let mut i18n = make_i18n("en", "en", langs);
        assert!(i18n.try_set_lang("fr").is_ok());
        assert_eq!(i18n.get_lang(), "fr");
    }

    #[test]
    fn available_languages_sorted() {
        let mut langs = LangMap::new();
        langs.insert("zh".into(), FileMap::new());
        langs.insert("en".into(), FileMap::new());
        langs.insert("fr".into(), FileMap::new());
        let i18n = make_i18n("en", "en", langs);
        let avail: Vec<&str> = i18n.available_languages().iter().map(String::as_str).collect();
        assert_eq!(avail, vec!["en", "fr", "zh"]);
    }
}
