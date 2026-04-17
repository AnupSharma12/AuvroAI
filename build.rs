use std::collections::HashMap;
use std::fs;

fn main() {
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-changed=build.rs");

    let env_content = fs::read_to_string(".env").unwrap_or_default();

    let required_keys = [
        "SUPABASE_URL",
        "SUPABASE_PUBLISHABLE_KEY",
        "AUVRO_API_KEY",
        "AUVRO_ENDPOINT",
        "AUVRO_MODEL",
        "OPENROUTER_API_KEY",
        "OPENROUTER_BASE_URL",
        "OPENROUTER_MODEL",
    ];

    let mut found: HashMap<String, String> = HashMap::new();

    for line in env_content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            found.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    let mut missing = Vec::new();
    for key in &required_keys {
        match found.get(*key) {
            Some(val) if !val.is_empty() => {
                println!("cargo:rustc-env={}={}", key, val);
            }
            _ => missing.push(*key),
        }
    }

    if !missing.is_empty() {
        panic!(
            "\n\n[build.rs] ERROR: Missing keys in .env file: {}\nCreate a .env file in the project root with all required keys.\n",
            missing.join(", ")
        );
    }

    #[cfg(target_os = "windows")]
    {
        if std::path::Path::new("assets/icon.ico").exists() {
            let mut res = winres::WindowsResource::new();
            res.set_icon("assets/icon.ico");
            res.compile().expect("Failed to compile Windows resources");
        }
    }
}
