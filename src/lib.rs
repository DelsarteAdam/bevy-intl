#![allow(dead_code)]
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

mod locales;

use serde::Deserialize;
use std::collections::{ HashMap };
use serde_json::Value;
use locales::LOCALES;
use regex::Regex;
use once_cell::sync::Lazy;

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
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            use_bundled_translations: cfg!(target_arch = "wasm32") || cfg!(feature = "bundle-only"),
            messages_folder: "messages".to_string(),
            default_lang: "en".to_string(),
            fallback_lang: "en".to_string(),
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
        app.insert_resource(self.config.clone()).init_resource::<I18n>();
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
    /// A nested map of key-value pairs (for plurals, genders, etc.)
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
}

impl FromWorld for I18n {
    fn from_world(world: &mut World) -> Self {
        let config = world.get_resource::<I18nConfig>().cloned().unwrap_or_default();

        let (translations, locale_folders_list) = if config.use_bundled_translations {
            load_bundled_translations()
        } else {
            load_filesystem_translations(&config.messages_folder)
        };

        Self {
            current_lang: config.default_lang,
            fallback_lang: config.fallback_lang,
            translations,
            locale_folders_list,
        }
    }
}

// ---------- Loaders ----------

// Chargement depuis le système de fichiers (mode dev/desktop)
#[cfg(not(target_arch = "wasm32"))]
fn load_filesystem_translations(messages_folder: &str) -> (Translations, Vec<String>) {
    match load_translation_from_fs(messages_folder) {
        Ok(langs) => {
            let locale_list = langs.keys().cloned().collect();
            (Translations { langs }, locale_list)
        }
        Err(e) => {
            eprintln!("⚠️ Failed to load translations from '{}': {}", messages_folder, e);
            create_error_translations()
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn load_filesystem_translations(_messages_folder: &str) -> (Translations, Vec<String>) {
    eprintln!("⚠️ Filesystem loading not available on WASM, using bundled translations");
    load_bundled_translations()
}

// Chargement depuis les traductions intégrées (bundle au build time)
fn load_bundled_translations() -> (Translations, Vec<String>) {
    match load_bundled_data() {
        Ok(langs) => {
            let locale_list = langs.keys().cloned().collect();
            (Translations { langs }, locale_list)
        }
        Err(e) => {
            eprintln!("⚠️ Failed to load bundled translations: {}", e);
            create_error_translations()
        }
    }
}

// Charge les données bundlées (générées par build.rs)
fn load_bundled_data() -> Result<LangMap, Box<dyn std::error::Error>> {
    const BUNDLED_TRANSLATIONS: &str = include_str!(
        concat!(env!("OUT_DIR"), "/all_translations.json")
    );
    let value: Value = serde_json::from_str(BUNDLED_TRANSLATIONS)?;
    parse_translation_value(value)
}

// Parse une Value JSON vers LangMap
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
                            let section_value = if let Some(text) = val.as_str() {
                                SectionValue::Text(text.to_string())
                            } else if let Some(nested) = val.as_object() {
                                let mut nested_map = HashMap::new();
                                for (nested_key, nested_val) in nested {
                                    if let Some(nested_str) = nested_val.as_str() {
                                        nested_map.insert(
                                            nested_key.clone(),
                                            nested_str.to_string()
                                        );
                                    }
                                }
                                SectionValue::Map(nested_map)
                            } else {
                                continue;
                            };
                            section_map.insert(key.clone(), section_value);
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

// Version système de fichiers
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
                        let section_value = if let Some(text) = value.as_str() {
                            SectionValue::Text(text.to_string())
                        } else if let Some(nested) = value.as_object() {
                            let mut nested_map = HashMap::new();
                            for (nested_key, nested_val) in nested {
                                if let Some(nested_str) = nested_val.as_str() {
                                    nested_map.insert(nested_key.clone(), nested_str.to_string());
                                }
                            }
                            SectionValue::Map(nested_map)
                        } else {
                            continue;
                        };
                        section_map.insert(key.clone(), section_value);
                    }
                }

                file_map.insert(file_name, section_map);
            }
        }

        lang_map.insert(lang_code, file_map);
    }

    Ok(lang_map)
}

// Traductions d'erreur par défaut
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

/// Extension trait for `App` to easily manage languages.
/// 
/// Provides convenient methods to set current and fallback languages
/// directly on the Bevy `App`.
/// 
/// # Example
/// 
/// ```rust
/// use bevy::prelude::*;
/// use bevy_intl::LanguageAppExt;
/// 
/// fn setup_language(mut app: ResMut<App>) {
///     app.set_lang_i18n("fr");
///     app.set_fallback_lang("en");
/// }
/// ```
pub trait LanguageAppExt {
    /// Sets the current language for translations.
    /// 
    /// Warns if the language is not available in loaded translations.
    fn set_lang_i18n(&mut self, locale: &str);
    /// Sets the fallback language for translations.
    /// 
    /// Warns if the fallback language is not available in loaded translations.
    fn set_fallback_lang(&mut self, locale: &str);
}

impl LanguageAppExt for App {
    fn set_lang_i18n(&mut self, locale: &str) {
        if let Some(mut i18n) = self.world_mut().get_resource_mut::<I18n>() {
            if !i18n.locale_folders_list.contains(&locale.to_string()) {
                warn!("Locale '{}' not found in available translations", locale);
                return;
            }
            i18n.current_lang = locale.to_string();
        }
    }

    fn set_fallback_lang(&mut self, locale: &str) {
        if let Some(mut i18n) = self.world_mut().get_resource_mut::<I18n>() {
            if !i18n.locale_folders_list.contains(&locale.to_string()) {
                warn!("Fallback locale '{}' not found in available translations", locale);
                return;
            }
            i18n.fallback_lang = locale.to_string();
        }
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
pub struct I18nPartial {
    /// Translations for the current language
    file_traductions: SectionMap,
    /// Fallback translations when current language is missing a key
    fallback_traduction: SectionMap,
}

impl I18n {
    /// Loads translations for a specific file.
    /// 
    /// Returns an `I18nPartial` that provides access to all translation
    /// methods for that file.
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
    pub fn translation(&self, translation_file: &str) -> I18nPartial {
        let error_map = {
            let mut map = HashMap::new();
            map.insert(
                "error".to_string(),
                SectionValue::Text("Translation not found".to_string())
            );
            map
        };

        // Traduction courante
        let current_file = self.translations.langs
            .get(&self.current_lang)
            .and_then(|lang| lang.get(translation_file))
            .cloned()
            .unwrap_or_else(|| error_map.clone());

        // Traduction de fallback
        let fallback_file = self.translations.langs
            .get(&self.fallback_lang)
            .and_then(|lang| lang.get(translation_file))
            .cloned()
            .unwrap_or(error_map);

        I18nPartial {
            file_traductions: current_file,
            fallback_traduction: fallback_file,
        }
    }

    /// Sets the current language.
    /// 
    /// # Arguments
    /// 
    /// * `locale` - Language code (e.g., "en", "fr", "es")
    /// 
    /// # Example
    /// 
    /// ```rust
    /// use bevy::prelude::*;
    /// use bevy_intl::I18n;
    /// 
    /// fn change_language(mut i18n: ResMut<I18n>) {
    ///     i18n.set_lang("fr");
    /// }
    /// ```
    pub fn set_lang(&mut self, locale: &str) {
        if !self.locale_folders_list.contains(&locale.to_string()) {
            warn!("Locale '{}' not available", locale);
            return;
        }
        self.current_lang = locale.to_string();
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
static ARG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{\{(\w*)\}\}").unwrap());

impl I18nPartial {
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
        self.get_text_value(key).unwrap_or_else(|| "Missing translation".to_string())
    }

    /// Gets a translated string with placeholder replacement.
    /// 
    /// Replaces `{{}}` placeholders in the translation with the provided arguments.
    /// 
    /// # Arguments
    /// 
    /// * `key` - Translation key to look up
    /// * `args` - Values to replace placeholders with
    /// 
    /// # Returns
    /// 
    /// The translated string with placeholders replaced.
    /// 
    /// # Example
    /// 
    /// ```rust
    /// // JSON: "welcome": "Hello {{name}}!"
    /// let text = i18n.translation("ui").t_with_arg("welcome", &[&"John"]);
    /// // Result: "Hello John!"
    /// ```
    pub fn t_with_arg(&self, key: &str, args: &[&dyn ToString]) -> String {
        let template = self.t(key);
        self.replace_placeholders(&template, args)
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
        // Try specific count first, then fallback to generic rules
        let count_str = count.to_string();
        
        // 1. Try exact count (e.g., "0", "1", "2", "3"...)
        if let Some(template) = self.get_nested_value(key, &count_str) {
            return self.replace_placeholders(&template, &[&count]);
        }
        
        // 2. Try standard plural categories
        let plural_key = match count {
            0 => "zero",    // Changed from "none" to match ICU standards
            1 => "one",
            2 => "two",
            3..=10 => "few",      // For languages like Polish, Russian
            _ => "many",
        };

        if let Some(template) = self.get_nested_value(key, plural_key) {
            return self.replace_placeholders(&template, &[&count]);
        }
        
        // 3. Fallback to basic English rules
        let basic_key = if count == 1 { "one" } else { "other" };
        if let Some(template) = self.get_nested_value(key, basic_key) {
            return self.replace_placeholders(&template, &[&count]);
        }
        
        // 4. Last resort fallbacks
        if let Some(template) = self.get_nested_value(key, "many") {
            return self.replace_placeholders(&template, &[&count]);
        }
        
        "Missing plural translation".to_string()
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

    /// Gets a gendered translation with placeholder replacement.
    /// 
    /// Combines gender selection and argument replacement.
    /// 
    /// # Arguments
    /// 
    /// * `key` - Translation key to look up
    /// * `gender` - Gender key
    /// * `args` - Values to replace placeholders with
    /// 
    /// # Returns
    /// 
    /// The translated string for the gender with placeholders replaced.
    /// 
    /// # Example
    /// 
    /// ```rust
    /// // JSON: "greeting": { "male": "Hello Mr. {{name}}", "female": "Hello Ms. {{name}}" }
    /// let text = i18n.translation("ui").t_with_gender_and_arg("greeting", "male", &[&"Smith"]);
    /// // Result: "Hello Mr. Smith"
    /// ```
    pub fn t_with_gender_and_arg(&self, key: &str, gender: &str, args: &[&dyn ToString]) -> String {
        let template = self.t_with_gender(key, gender);
        self.replace_placeholders(&template, args)
    }

    // Méthodes utilitaires privées
    fn get_text_value(&self, key: &str) -> Option<String> {
        self.file_traductions
            .get(key)
            .and_then(|v| if let SectionValue::Text(s) = v { Some(s.clone()) } else { None })
            .or_else(|| {
                self.fallback_traduction
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
        self.file_traductions
            .get(key)
            .and_then(|v| (
                if let SectionValue::Map(m) = v {
                    m.get(nested_key).cloned()
                } else {
                    None
                }
            ))
            .or_else(|| {
                self.fallback_traduction
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

    fn replace_placeholders(&self, template: &str, args: &[&dyn ToString]) -> String {
        let parts: Vec<&str> = ARG_RE.split(template).collect();
        let mut result = String::new();

        for (i, part) in parts.iter().enumerate() {
            result.push_str(part);
            if i < args.len() {
                result.push_str(&args[i].to_string());
            }
        }

        result
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
