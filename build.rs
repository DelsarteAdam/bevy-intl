use std::error::Error;
use std::{ fs, path::Path };
use serde_json::{ Value, Map };
use anyhow::Result;

fn main() -> Result<(), Box<dyn Error>> {
    let messages_dir = Path::new("messages");
    let out_path = Path::new(&std::env::var("OUT_DIR")?).join("all_translations.json");

    // Always create the file, even if empty, so include_str! works
    if !messages_dir.exists() {
        println!("cargo:warning=No messages/ folder found, creating empty translations");
        fs::write(out_path, "{}")?;
        return Ok(());
    }

    let translations = build_translations(messages_dir)?;
    fs::write(out_path, serde_json::to_string_pretty(&translations)?)?;

    println!("cargo:rerun-if-changed=messages");
    Ok(())
}

fn build_translations(messages_dir: &Path) -> Result<Value> {
    let mut translations = Map::new();

    for lang_entry in fs::read_dir(messages_dir)? {
        let lang_dir = lang_entry?;
        if !lang_dir.file_type()?.is_dir() {
            continue;
        }

        let lang_code = lang_dir.file_name().to_string_lossy().to_string();
        let mut translation_files = Map::new();

        for file_entry in fs::read_dir(lang_dir.path())? {
            let file = file_entry?;
            let file_path = file.path(); // Store the path to extend its lifetime

            if let Some("json") = file_path.extension().and_then(|e| e.to_str()) {
                let file_stem = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                let content = fs::read_to_string(&file_path)?;
                let json: Value = serde_json::from_str(&content)?;
                translation_files.insert(file_stem.to_string(), json);
            }
        }
        translations.insert(lang_code, Value::Object(translation_files));
    }

    Ok(Value::Object(translations))
}
