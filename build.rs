use std::error::Error;
use std::{ fs, path::Path, path::PathBuf };
use serde_json::{ Value, Map };
use anyhow::Result;

fn main() -> Result<(), Box<dyn Error>> {
    // Try to find messages directory in the consuming project
    let messages_dir = find_messages_directory()?;
    let out_path = Path::new(&std::env::var("OUT_DIR")?).join("all_translations.json");

    // Always create the file, even if empty, so include_str! works
    if !messages_dir.exists() {
        println!("cargo:warning=No messages/ folder found in consuming project");
        println!("cargo:warning=This is normal when building bevy-intl itself");
        fs::write(out_path, "{}")?;
        return Ok(());
    }

    let translations = build_translations(&messages_dir)?;
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

fn find_messages_directory() -> Result<PathBuf> {
    // First try the workspace root (if CARGO_TARGET_DIR is set)
    if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        let workspace_root = Path::new(&target_dir)
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid target dir"))?;
        let messages_path = workspace_root.join("messages");
        if messages_path.exists() {
            return Ok(messages_path);
        }
    }

    // Try current working directory
    let cwd_messages = Path::new("messages");
    if cwd_messages.exists() {
        return Ok(cwd_messages.to_path_buf());
    }

    // Try parent directories up to root
    let mut current = std::env::current_dir()?;
    loop {
        let messages_path = current.join("messages");
        if messages_path.exists() {
            return Ok(messages_path);
        }

        if !current.pop() {
            break;
        }
    }

    // Fallback to messages in current directory (even if it doesn't exist)
    Ok(Path::new("messages").to_path_buf())
}
