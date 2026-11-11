//! Bundles the consuming project's translations into the binary.
//!
//! Cargo runs a build script in the directory of the crate that owns it, i.e.
//! bevy-intl itself (or its copy in `~/.cargo/registry`), never in the game.
//! The script therefore cannot find the game's `messages/` folder by itself:
//! the game tells it where the folder is through `BEVY_INTL_MESSAGES_DIR`,
//! usually set in its `.cargo/config.toml`:
//!
//! ```toml
//! [env]
//! BEVY_INTL_MESSAGES_DIR = { value = "messages", relative = true }
//! ```
//!
//! `relative = true` makes Cargo turn the value into an absolute path, relative
//! to the folder that contains `.cargo/`.

use anyhow::{ Context, Result, bail };
use serde_json::{ Map, Value };
use std::{ env, fs, path::{ Path, PathBuf } };

const MESSAGES_DIR_VAR: &str = "BEVY_INTL_MESSAGES_DIR";

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed={MESSAGES_DIR_VAR}");

    let out_path = Path::new(&env::var("OUT_DIR")?).join("all_translations.json");

    // Bundled translations are only used by default on WASM or with the
    // `bundle-only` feature: only then is a missing folder worth a warning.
    let bundling_expected = env::var_os("CARGO_FEATURE_BUNDLE_ONLY").is_some() ||
        env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");

    let Some(messages_dir) = env::var_os(MESSAGES_DIR_VAR).map(PathBuf::from) else {
        if bundling_expected {
            println!(
                "cargo:warning={MESSAGES_DIR_VAR} is not set: no translations bundled. \
                 Add `[env] {MESSAGES_DIR_VAR} = {{ value = \"messages\", relative = true }}` \
                 to your project's .cargo/config.toml"
            );
        }
        // Always create the file, even if empty, so `include_str!` works.
        fs::write(out_path, "{}")?;
        return Ok(());
    };

    if !messages_dir.is_absolute() {
        bail!(
            "{MESSAGES_DIR_VAR} must be an absolute path (got '{}'); in .cargo/config.toml use \
             `{MESSAGES_DIR_VAR} = {{ value = \"{}\", relative = true }}`",
            messages_dir.display(),
            messages_dir.display()
        );
    }
    if !messages_dir.is_dir() {
        bail!("{MESSAGES_DIR_VAR} points to '{}', which is not a folder", messages_dir.display());
    }

    // For a folder, Cargo rescans its whole content for changes.
    println!("cargo:rerun-if-changed={}", messages_dir.display());

    let translations = build_translations(&messages_dir)?;
    fs::write(out_path, serde_json::to_string_pretty(&translations)?)?;
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
            let file_path = file_entry?.path();
            if file_path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let file_stem = file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");

            let content = fs
                ::read_to_string(&file_path)
                .with_context(|| format!("reading {}", file_path.display()))?;
            let json: Value = serde_json
                ::from_str(&content)
                .with_context(|| format!("invalid JSON in {}", file_path.display()))?;
            translation_files.insert(file_stem.to_string(), json);
        }
        translations.insert(lang_code, Value::Object(translation_files));
    }

    Ok(Value::Object(translations))
}
