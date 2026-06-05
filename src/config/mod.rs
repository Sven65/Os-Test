use alloc::string::{String, ToString};
use crate::{fs, serial_println};

const CONFIG_FILE: &str = "system.ini";

#[derive(Debug)]
pub struct SystemConfig {
    pub hostname: String,
    pub keyboard_layout: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            hostname: String::from("myos"),
            keyboard_layout: String::from("us"),
        }
    }
}

impl SystemConfig {
    pub fn load() -> Self {
        let mut config = SystemConfig::default();

        let data = match fs::read_file(CONFIG_FILE) {
            Some(d) => d,
            None => {
                serial_println!("[config] No config file found, using defaults");
                return config;
            }
        };

        let text = match core::str::from_utf8(&data) {
            Ok(t) => t,
            Err(_) => return config,
        };

        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                match key.trim() {
                    "hostname" => config.hostname = value.trim().to_string(),
                    "keyboard_layout" => config.keyboard_layout = value.trim().to_string(),
                    _ => serial_println!("[config] Unknown key: {}", key.trim()),
                }
            }
        }

        serial_println!("[config] Loaded: {:#?}", config);
        config
    }

    pub fn save(&self) -> bool {
        let contents = alloc::format!(
            "# System configuration\nhostname={}\nkeyboard_layout={}\n",
            self.hostname,
            self.keyboard_layout,
        );
        fs::write_file(CONFIG_FILE, contents.as_bytes())
    }
}