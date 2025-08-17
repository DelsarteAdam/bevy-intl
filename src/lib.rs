#![allow(dead_code)]
/*! # bevy-intl

bevy-intl` is an internationalization (i18n) plugin for Bevy.
It allows you to load translations, handle plurals, genders, and placeholders,
and easily switch languages at runtime in your Bevy applications.

!*/

use bevy::prelude::*;

mod locales;

use serde::Deserialize;
use std::collections::{ HashMap, HashSet };
use std::fs;
use std::path::PathBuf;
use serde_json::Value;
use locales::LOCALES;
use regex::Regex;
use once_cell::sync::Lazy;

/// Represents a value in a translation section, which can either
/// be a simple text string or a nested map of key-value pairs.
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum SectionValue {
    Text(String),
    Map(HashMap<String, String>),
}

/// A mapping of translation keys to their values within a file.
type SectionMap = HashMap<String, SectionValue>;
/// A mapping of file names to their section maps.
type FileMap = HashMap<String, SectionMap>;
/// A mapping of language codes to file maps.
type LangMap = HashMap<String, FileMap>;

/// Contains all translations loaded from disk.
#[derive(Debug, Deserialize)]
pub struct Translations {
    pub langs: LangMap,
}

// ---------- Bevy Plugin ----------

/// Main plugin for Bevy internationalization.
///
/// Handles language switching, loading translation files,
/// and providing `Translation` objects for accessing localized strings.
pub fn plugin(app: &mut App) {
    app.init_resource::<I18n>();
}

/// Resource that stores translations and language settings.
#[derive(Resource)]
pub struct I18n {
    translations: Translations,
    current_lang: String,
    locale_folders_list: Vec<String>,
    fallback_lang: String,
}

impl Default for I18n {
    /// Loads translations and folder list at startup.
    fn default() -> Self {
        let translations = Translations {
            langs: load_translation().unwrap_or_else(|e| {
                eprintln!("⚠️ Failed to load translations from the 'messages' folder: {e}");
                let mut section_map = HashMap::new();
                section_map.insert("error".to_string(), SectionValue::Text("error".to_string()));
                let mut file_map = HashMap::new();
                file_map.insert("error".to_string(), section_map);
                let mut lang_map = HashMap::new();
                lang_map.insert("error".to_string(), file_map);

                lang_map
            }),
        };

        let locale_folders_list = get_folder_locale_list().unwrap_or_else(|e| {
            eprintln!("⚠️ Failed to load folder locale list from the 'messages' folder: {e}");
            vec![]
        });

        Self {
            current_lang: "en".to_string(),
            translations,
            locale_folders_list,
            fallback_lang: "en".to_string(),
        }
    }
}

// ---------- Loaders ----------

/// Loads translation files from the `messages` folder and constructs a `LangMap`.
/// Checks for missing files and validates folder structure.
fn load_translation() -> std::io::Result<LangMap> {
    //check translation symetry for missing file/folder
    check_for_missing_file();
    //find messages folder at the root project
    let mut message_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    message_dir.push("messages");

    let mut langhash: LangMap = HashMap::new();

    //loop folder structure
    if message_dir.is_dir() {
        for folder in fs::read_dir(message_dir)?.filter_map(|entry| entry.ok()) {
            let lang_folder = folder.file_name().to_string_lossy().to_string();
            let mut filehash: FileMap = HashMap::new();

            for file in fs
                ::read_dir(folder.path())?
                .filter_map(|entry| entry.ok()) // keep only successful DirEntry
                .filter(|entry| {
                    entry.path().is_file() &&
                        entry
                            .path()
                            .extension()
                            .and_then(|ext| ext.to_str()) == Some("json")
                }) {
                let file_name = file
                    .path()
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();

                //insert all content of json into HashMap<String, String>
                let mut sectionhash: SectionMap = HashMap::new();
                let data = fs::read_to_string(file.path())?;
                let json: Value = serde_json::from_str(&data)?;

                if let Some(obj) = json.as_object() {
                    for (key, value) in obj {
                        if let Some(val_str) = value.as_str() {
                            // simple string
                            sectionhash.insert(
                                key.clone(),
                                SectionValue::Text(val_str.to_string())
                            );
                        } else if let Some(val_obj) = value.as_object() {
                            // nested map
                            let mut nested_map = HashMap::new();
                            for (nested_key, nested_val) in val_obj {
                                if let Some(nested_str) = nested_val.as_str() {
                                    nested_map.insert(nested_key.clone(), nested_str.to_string());
                                }
                            }
                            sectionhash.insert(key.clone(), SectionValue::Map(nested_map));
                        }
                    }
                }

                //insert to filehash and langhash
                filehash.insert(file_name, sectionhash);
            }
            langhash.insert(lang_folder, filehash);
        }
    } else {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "messages folder not found"));
    }

    if cfg!(debug_assertions) {
        println!("\x1b[33mtranslation files loaded\x1b[0m");
    }

    Ok(langhash)
}

/// Returns a list of locale folder names inside the `messages` folder.
/// Validates each folder against the international standard.
fn get_folder_locale_list() -> std::io::Result<Vec<String>> {
    //find messages folder at the root project
    let mut message_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    message_dir.push("messages");

    let mut locale_list = Vec::new();

    if message_dir.is_dir() {
        for folder in fs::read_dir(message_dir)?.filter_map(|entry| entry.ok()) {
            let lang_folder = folder.file_name().to_string_lossy().to_string();
            let lang_warning: bool = locale_exists_as_international_standard(&lang_folder);
            if cfg!(debug_assertions) && !lang_warning {
                println!(
                    "\x1b[33m⚠️  locale {lang_folder} maybe does not exist does not exist as a international standard\x1b[0m"
                );
            }
            locale_list.push(lang_folder);
        }
    } else {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "messages folder not found"));
    }

    if cfg!(debug_assertions) {
        println!("\x1b[33mlocale list loaded\x1b[0m");
    }

    Ok(locale_list)
}

// ---------- API ----------

/// Extension trait for `App` to set current and fallback languages.
pub trait LanguageAppExt {
    /// Sets the current language for translations.
    fn set_lang_i18n(&mut self, locale: &str);
    /// Sets the fallback language for translations.
    fn set_fallback_lang(&mut self, locale: &str);
}

impl LanguageAppExt for App {
    fn set_lang_i18n(&mut self, locale: &str) {
        if let Some(mut i18n) = self.world_mut().get_resource_mut::<I18n>() {
            if !i18n.locale_folders_list.contains(&locale.to_string()) {
                if cfg!(debug_assertions) {
                    eprintln!("\x1b[33m⚠️  locale '{}' does not exist in messages folder\x1b[0m", locale);
                }
                return;
            }
            i18n.current_lang = locale.to_string();
        }
    }

    fn set_fallback_lang(&mut self, locale: &str) {
        if let Some(mut i18n) = self.world_mut().get_resource_mut::<I18n>() {
            if !i18n.locale_folders_list.contains(&locale.to_string()) {
                if cfg!(debug_assertions) {
                    eprintln!("\x1b[33m⚠️  locale '{}' does not exist in messages folder\x1b[0m", locale);
                }
                return;
            }
            i18n.fallback_lang = locale.to_string();
        }
    }
}

// ---------- Translation Handling ----------

/// Represents a partial translation, i.e., translations for a single file.
pub struct I18nPartial {
    file_traductions: SectionMap,
    fallback_traduction: SectionMap,
}

impl I18n {
    /// Returns an `I18nPartial` for a specific translation file.
    pub fn translation(&self, translation_file: &str) -> I18nPartial {
        let mut error_map = HashMap::new();
        error_map.insert("error".to_string(), SectionValue::Text("error".to_string()));
        // Try current language
        let lang_traduction = self.translations.langs
            .get(&self.current_lang)
            .expect("Language not found");

        let section_file = lang_traduction.get(translation_file);

        // Fallback language
        let fallback_lang_traduction = self.translations.langs
            .get(&self.fallback_lang)
            .expect("Fallback language not found");

        let fallback_section_file = fallback_lang_traduction
            .get(translation_file)
            .cloned()
            .unwrap_or_else(|| {
                println!(
                    "\x1b[33m⚠️ Failed to load translations from the 'messages' folder\x1b[0m"
                );
                error_map
            });

        // Use current translation if available, otherwise fallback
        let final_section_file = section_file.unwrap_or(&fallback_section_file);

        I18nPartial {
            file_traductions: final_section_file.clone(),
            fallback_traduction: fallback_section_file,
        }
    }

    /// Changes the current language at runtime.
    pub fn set_lang(&mut self, locale: &str) {
        if !self.locale_folders_list.contains(&locale.to_string()) {
            if cfg!(debug_assertions) {
                eprintln!("\x1b[33mWARNING: locale '{}' does not exist in messages folder\x1b[0m", locale);
            }

            return;
        }
        self.current_lang = locale.to_string();
    }

    /// Returns the currently active language.
    pub fn get_lang(&self) -> String {
        self.current_lang.clone()
    }
}

// ---------- Text helpers ----------
static ARG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\{\{(\w*)\}\}").unwrap());

impl I18nPartial {
    /// Returns a translated string by key.
    pub fn t(&self, translation_line: &str) -> String {
        let get_text = |map: &SectionMap| {
            map.get(translation_line).and_then(|v| {
                if let SectionValue::Text(s) = v { Some(s.clone()) } else { None }
            })
        };

        get_text(&self.file_traductions)
            .or_else(|| get_text(&self.fallback_traduction))
            .unwrap_or_else(|| "Error missing text".to_owned())
    }

    /// Returns a translated string and replaces placeholders with provided arguments.
    pub fn t_with_arg(&self, translation_line: &str, arguments: &[&dyn ToString]) -> String {
        let original_line = self.t(translation_line);
        let vec_line: Vec<&str> = ARG_RE.split(&original_line).collect();
        let mut line_rebuild = String::new();

        for (i, part) in vec_line.iter().enumerate() {
            line_rebuild.push_str(part);
            if i < arguments.len() {
                line_rebuild.push_str(&arguments[i].to_string());
            }
        }

        line_rebuild
    }

    /// Returns a pluralized translation based on `count`.
    pub fn t_with_plurial(&self, translation_line: &str, count: usize) -> String {
        let get_hash = |map: &SectionMap| {
            map.get(translation_line).and_then(|v| {
                if let SectionValue::Map(s) = v { Some(s.clone()) } else { None }
            })
        };

        // Closure to get the line from the nested map
        let get_line = |line: &str| -> String {
            get_hash(&self.file_traductions)
                .and_then(|hash| hash.get(line).cloned())
                .or_else(||
                    get_hash(&self.fallback_traduction).and_then(|hash| hash.get(line).cloned())
                )
                .unwrap_or_else(|| "Error missing text".to_owned())
        };

        let match_line = match count {
            0 => get_line("none"),
            1 => get_line("one"),
            _ => get_line("many"),
        };

        //simple translation with only 1 arg

        let vec_line: Vec<&str> = ARG_RE.split(&match_line).collect();
        let mut line_rebuild = String::new();
        let mut is_arg_inserted = false;

        for part in vec_line.iter() {
            line_rebuild.push_str(part);
            if !is_arg_inserted {
                line_rebuild.push_str(&count.to_string());
                is_arg_inserted = true;
            }
        }

        line_rebuild
    }

    /// Returns a gender-specific translation based on `gender`.
    pub fn t_with_gender(&self, translation_line: &str, gender: &str) -> String {
        let get_hash = |map: &SectionMap| {
            map.get(translation_line).and_then(|v| {
                if let SectionValue::Map(s) = v { Some(s.clone()) } else { None }
            })
        };

        // Closure to get the line from the nested map
        let get_line = |line: &str| -> String {
            get_hash(&self.file_traductions)
                .and_then(|hash| hash.get(line).cloned())
                .or_else(||
                    get_hash(&self.fallback_traduction).and_then(|hash| hash.get(line).cloned())
                )
                .unwrap_or_else(|| "Error missing text".to_owned())
        };

        get_line(gender)
    }

    /// Returns a gender-specific translation with arguments replaced.
    pub fn t_with_gender_and_arg(
        &self,
        translation_line: &str,
        gender: &str,
        arguments: &[&dyn ToString]
    ) -> String {
        let line = self.t_with_gender(translation_line, gender);

        let vec_line: Vec<&str> = ARG_RE.split(&line).collect();
        let mut line_rebuild = String::new();

        for (i, part) in vec_line.iter().enumerate() {
            line_rebuild.push_str(part);
            if i < arguments.len() {
                line_rebuild.push_str(&arguments[i].to_string());
            }
        }

        line_rebuild
    }
}

// ---------- Utils ----------

/// Checks if a locale string exists as an international standard.
fn locale_exists_as_international_standard(locale: &str) -> bool {
    LOCALES.binary_search(&locale).is_ok()
}

/// Checks for missing translation files across all language folders.
fn check_for_missing_file() {
    // find messages folder at the root project
    let mut message_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    message_dir.push("messages");

    if !message_dir.is_dir() {
        println!("\x1b[33mWARNING: There is no messages folder\x1b[0m");
        return;
    }

    let mut folder_list = Vec::new();

    // Iterate only over Ok entries
    for folder in fs
        ::read_dir(&message_dir)
        .unwrap_or_else(|_| {
            println!("\x1b[31mERROR: Failed to read messages folder\x1b[0m");
            fs::read_dir(".").unwrap()
        })
        .filter_map(Result::ok) {
        let lang_folder = folder.file_name().to_string_lossy().to_string();
        let mut file_list = Vec::new();

        for file in fs
            ::read_dir(folder.path())
            .unwrap_or_else(|_| fs::read_dir(".").unwrap())
            .filter_map(Result::ok) {
            let path = file.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                let file_name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                file_list.push(file_name);
            }
        }

        folder_list.push((lang_folder, file_list));
    }

    // collect all unique files across all folders
    let mut all_files = HashSet::new();
    for (_, files) in &folder_list {
        for file in files {
            all_files.insert(file);
        }
    }

    // check each folder for missing files
    for (folder, files) in &folder_list {
        let file_set: HashSet<_> = files.iter().collect();
        for file in &all_files {
            if !file_set.contains(file) {
                println!("\x1b[31mWarning: Folder '{}' is missing file '{}'\x1b[0m", folder, file);
            }
        }
    }
}
