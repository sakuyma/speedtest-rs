//! Persistent settings and license acceptance handling.

use crate::config::Config;
use crate::json::{self, Value};
use std::io::Write;

#[derive(Debug, Default, Clone)]
pub struct Settings {
    pub license_accepted: String,
    pub gdpr_accepted: String,
}

fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_default()
}

fn ensure_parent_dirs(path: &str) {
    // Create every directory component except the final filename.
    let chars: Vec<char> = path.chars().collect();
    for i in 1..chars.len() {
        if chars[i] == '/' {
            let dir: String = chars[..i].iter().collect();
            if !dir.is_empty() {
                let _ = std::fs::create_dir_all(&dir);
            }
        }
    }
}

/// $XDG_CONFIG_HOME or ~/.config, then /ookla/speedtest-cli.json
pub fn settings_path() -> String {
    let base = match std::env::var("XDG_CONFIG_HOME") {
        Ok(x) if !x.is_empty() => x,
        _ => format!("{}/.config", home_dir()),
    };
    format!("{base}/ookla/speedtest-cli.json")
}

pub fn load_settings() -> Settings {
    let mut s = Settings::default();
    let content = match std::fs::read_to_string(settings_path()) {
        Ok(c) => c,
        Err(_) => return s,
    };
    let root = match json::parse(&content) {
        Ok(v) => v,
        Err(_) => return s,
    };
    let st = match root.find("Settings") {
        Some(v) => v,
        None => return s,
    };
    s.license_accepted = st.get_string("LicenseAccepted", "");
    s.gdpr_accepted = st.get_string("GdprAccepted", "");
    s
}

pub fn save_settings(s: &Settings) {
    let mut root = Value::object();
    let mut st = Value::object();
    if !s.license_accepted.is_empty() {
        st.set("LicenseAccepted", Value::string(s.license_accepted.clone()));
    }
    if !s.gdpr_accepted.is_empty() {
        st.set("GdprAccepted", Value::string(s.gdpr_accepted.clone()));
    }
    root.set("Settings", st);

    let path = settings_path();
    ensure_parent_dirs(&path);
    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = writeln!(f, "{}", root.dump(true));
    }
}

/// The value persisted when the license is accepted (tied to license version).
pub fn license_token(cfg: &Config) -> String {
    if cfg.license_version.is_empty() {
        "1".to_string()
    } else {
        cfg.license_version.clone()
    }
}

/// Format the license banner (message) wrapped between '=' rules.
pub fn license_banner(message: &str) -> String {
    let rule = "=".repeat(78);
    format!("{rule}\n\n{message}\n\n{rule}\n")
}
