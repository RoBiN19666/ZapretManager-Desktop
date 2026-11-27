//! Настройки самого менеджера: data\manager.ini (формат key=value, как в C#-версии).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::textfile;

#[derive(Debug, Clone)]
pub struct Ini {
    pub path: PathBuf,
    /// ключ в нижнем регистре → (исходное написание, значение)
    values: BTreeMap<String, (String, String)>,
    /// ключи, изменённые этим экземпляром: только их переносим при сохранении,
    /// чтобы не затереть настройки, записанные параллельно другим режимом.
    dirty: BTreeSet<String>,
}

impl Ini {
    pub fn load(path: &Path) -> Ini {
        let mut ini = Ini {
            path: path.to_path_buf(),
            values: BTreeMap::new(),
            dirty: BTreeSet::new(),
        };
        for raw in textfile::read_lines(path) {
            let line = raw.trim();
            if line.is_empty()
                || line.starts_with('#')
                || line.starts_with('[')
                || line.starts_with(';')
            {
                continue;
            }
            if let Some(eq) = line.find('=') {
                if eq == 0 {
                    continue;
                }
                let key = line[..eq].trim();
                let value = line[eq + 1..].trim();
                ini.values
                    .insert(key.to_lowercase(), (key.to_string(), value.to_string()));
            }
        }
        ini
    }

    pub fn save(&self) -> std::io::Result<()> {
        // Перечитываем файл с диска: параллельный режим (headless, TG-прокси,
        // второй экземпляр) мог сохранить свои ключи. Переносим только то,
        // что изменили мы, остальное берём с диска.
        let mut merged = Ini::load(&self.path).values;
        if merged.is_empty() {
            merged = self.values.clone();
        }
        for key in &self.dirty {
            if let Some(value) = self.values.get(key) {
                merged.insert(key.clone(), value.clone());
            }
        }
        let mut keys: Vec<&String> = merged.keys().collect();
        keys.sort();
        let mut out = String::new();
        out.push_str("# Zapret Manager — настройки. Файл можно редактировать вручную.\n");
        for lower in keys {
            let (name, value) = &merged[lower];
            out.push_str(name);
            out.push('=');
            out.push_str(value);
            out.push('\n');
        }
        textfile::write_text_atomic(&self.path, &out)
    }

    pub fn get(&self, key: &str, default: &str) -> String {
        self.values
            .get(&key.to_lowercase())
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| default.to_string())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        let lower = key.to_lowercase();
        let name = self
            .values
            .get(&lower)
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| key.to_string());
        self.dirty.insert(lower.clone());
        self.values.insert(lower, (name, value.to_string()));
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        let v = self.get(key, if default { "1" } else { "0" }).to_lowercase();
        matches!(v.trim(), "1" | "true" | "yes" | "on")
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.set(key, if value { "1" } else { "0" });
    }

    pub fn get_int(&self, key: &str, default: i64) -> i64 {
        self.get(key, "").trim().parse().unwrap_or(default)
    }

    pub fn set_int(&mut self, key: &str, value: i64) {
        self.set(key, &value.to_string());
    }

    pub fn set_default(&mut self, key: &str, value: &str) {
        if !self.values.contains_key(&key.to_lowercase()) {
            self.set(key, value);
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_does_not_drop_foreign_keys() {
        let path =
            std::env::temp_dir().join(format!("zapret-ini-merge-{}.ini", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut a = Ini::load(&path);
        a.set("Root", r"C:\zapret");
        let mut b = Ini::load(&path);
        b.set("Theme", "dark");
        b.save().unwrap();
        // «a» сохраняет уже после «b»: ключ Theme не должен пропасть.
        a.save().unwrap();
        let after = Ini::load(&path);
        assert_eq!(after.get("Root", ""), r"C:\zapret");
        assert_eq!(after.get("Theme", ""), "dark");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn ini_roundtrip() {
        let path = std::env::temp_dir().join(format!("zapret-ini-test-{}.ini", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut ini = Ini::load(&path);
        ini.set("Root", r"C:\zapret");
        ini.set_bool("CloseToTray", false);
        ini.set_int("UiScale", 90);
        ini.save().unwrap();
        let loaded = Ini::load(&path);
        assert_eq!(loaded.get("root", ""), r"C:\zapret");
        assert!(!loaded.get_bool("CloseToTray", true));
        assert_eq!(loaded.get_int("UiScale", 100), 90);
        let _ = std::fs::remove_file(&path);
    }
}
