//! Списки доменов: анализ, чистка, пометка своих изменений в официальных списках.

use std::collections::HashSet;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use super::textfile;

pub const OFFICIAL_FILES: [&str; 4] = [
    "list-general.txt",
    "list-google.txt",
    "list-exclude.txt",
    "ipset-exclude.txt",
];

pub const BLOCK_START: &str = "# >>> Zapret Manager: мои добавления (сохраняются при обновлении zapret)";
pub const BLOCK_END: &str = "# <<< Zapret Manager: конец моих добавлений";

const MARK_START_PREFIX: &str = "# >>> Zapret Manager";
const MARK_END_PREFIX: &str = "# <<< Zapret Manager";

pub fn is_official(name: &str) -> bool {
    OFFICIAL_FILES.iter().any(|f| f.eq_ignore_ascii_case(name))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    Wildcard,
    Duplicate,
}

#[derive(Debug, Clone)]
pub struct ListNote {
    pub line: usize,
    pub entry: String,
    pub kind: NoteKind,
}

/// Нормализованная запись списка; "" для комментариев и пустых строк.
pub fn normalize(line: &str) -> String {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') || t.starts_with(';') || t.starts_with('/') {
        return String::new();
    }
    t.to_lowercase()
}

pub fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with('#') || t.starts_with(';') || t.starts_with('/')
}

pub fn analyze(text: &str) -> Vec<ListNote> {
    let mut notes = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (idx, raw) in text.replace("\r\n", "\n").split('\n').enumerate() {
        let entry = normalize(raw);
        if entry.is_empty() {
            continue;
        }
        if entry.starts_with("*.") {
            notes.push(ListNote {
                line: idx,
                entry: entry.clone(),
                kind: NoteKind::Wildcard,
            });
        }
        if !seen.insert(entry.clone()) {
            notes.push(ListNote {
                line: idx,
                entry,
                kind: NoteKind::Duplicate,
            });
        }
    }
    notes
}

pub fn count_kind(notes: &[ListNote], kind: NoteKind) -> usize {
    notes.iter().filter(|n| n.kind == kind).count()
}

pub fn remove_duplicates(text: &str) -> (String, usize) {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut removed = 0usize;
    for raw in text.replace("\r\n", "\n").split('\n') {
        let entry = normalize(raw);
        if entry.is_empty() {
            out.push(raw.to_string());
            continue;
        }
        if !seen.insert(entry) {
            removed += 1;
            continue;
        }
        out.push(raw.to_string());
    }
    (out.join("\n"), removed)
}

pub fn fix_wildcards(text: &str) -> (String, usize) {
    let mut fixed = 0usize;
    let mut out: Vec<String> = Vec::new();
    for raw in text.replace("\r\n", "\n").split('\n') {
        let trimmed_start = raw.trim_start();
        if trimmed_start.starts_with("*.") && trimmed_start.len() > 2 {
            let indent = &raw[..raw.len() - trimmed_start.len()];
            out.push(format!("{}{}", indent, &trimmed_start[2..]));
            fixed += 1;
        } else {
            out.push(raw.to_string());
        }
    }
    (out.join("\n"), fixed)
}

fn trim_punctuation(s: &str) -> &str {
    s.trim_matches(|c: char| {
        matches!(
            c,
            '"' | '\'' | '`' | '<' | '>' | '(' | ')' | '[' | ']' | ',' | ';' | '|'
        )
    })
}

/// Очистка домена из произвольной строки (ссылки, пути, порты, кавычки).
pub fn clean_token(raw: &str) -> String {
    let mut s = trim_punctuation(raw.trim()).replace('\\', "/");
    if let Some(idx) = s.find("://") {
        s = s[idx + 3..].to_string();
    } else if let Some(rest) = s.strip_prefix("//") {
        s = rest.to_string();
    }
    for stop in ['/', '?', '#'] {
        if let Some(idx) = s.find(stop) {
            s.truncate(idx);
        }
    }
    if let Some(at) = s.find('@') {
        s = s[at + 1..].to_string();
    }
    if let Some(rest) = s.strip_prefix("*.") {
        s = rest.to_string();
    }
    if let Some(rest) = s.strip_prefix('^') {
        s = rest.to_string();
    }
    if let Some(idx) = s.find(':') {
        if idx > 0 {
            s.truncate(idx);
        }
    }
    s.trim_end_matches('.').to_lowercase()
}

/// Допустимое доменное имя (или IP). Уже в нижнем регистре.
pub fn is_host_like(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    if !host.contains('.') || host.starts_with('.') || host.ends_with('.') || host.contains("..") {
        return false;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    for label in &labels {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return false;
        }
    }
    let tld = labels.last().unwrap();
    tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_lowercase())
}

/// Очистка IP-подсети: "203.0.113.113/32", "10.0.0.0/8".
pub fn clean_ip_token(raw: &str) -> String {
    let mut s = trim_punctuation(raw.trim()).to_string();
    if let Some(idx) = s.find('#') {
        s.truncate(idx);
    }
    if let Some(idx) = s.find([' ', '\t']) {
        if idx > 0 {
            s.truncate(idx);
        }
    }
    let (ip_part, prefix) = match s.split_once('/') {
        Some((ip, mask)) => (ip, Some(mask)),
        None => (s.as_str(), None),
    };
    let Ok(ip) = ip_part.parse::<IpAddr>() else {
        return String::new();
    };
    if let Some(mask) = prefix {
        let Ok(bits) = mask.parse::<u32>() else {
            return String::new();
        };
        let max = if ip.is_ipv4() { 32 } else { 128 };
        if bits > max {
            return String::new();
        }
        format!("{}/{}", ip, bits)
    } else {
        ip.to_string()
    }
}

fn clean_host_token(raw: &str) -> String {
    let anchor = raw.trim_start().starts_with('^');
    let host = clean_token(raw);
    if host.is_empty() || !is_host_like(&host) {
        return String::new();
    }
    if anchor {
        format!("^{}", host)
    } else {
        host
    }
}

/// Разбор массовой вставки: любые разделители, ссылки и мусор вычищаются.
pub fn parse_batch(text: &str, ip_mode: bool) -> (Vec<String>, usize) {
    let mut result: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut skipped = 0usize;
    let tokens: Vec<&str> = text
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '|'))
        .filter(|t| !t.is_empty())
        .collect();
    for raw in tokens {
        if ip_mode {
            let host = clean_ip_token(raw);
            if host.is_empty() {
                skipped += 1;
                continue;
            }
            let lower = host.to_lowercase();
            if !seen.insert(lower.clone()) {
                skipped += 1;
                continue;
            }
            result.push(host);
        } else {
            let anchor = raw.trim_start().starts_with('^');
            let mut host = clean_token(raw);
            if anchor && !host.is_empty() {
                host = format!("^{}", host);
            }
            let bare = host.trim_start_matches('^');
            if host.is_empty() || !is_host_like(bare) {
                if !raw.trim().is_empty() {
                    skipped += 1;
                }
                continue;
            }
            let key = host.to_lowercase();
            if !seen.insert(key) {
                skipped += 1;
                continue;
            }
            result.push(host);
        }
    }
    (result, skipped)
}

/// «Причесать строки»: убрать мусор, повторы и невалидные строки.
pub fn tidy(text: &str, ip_mode: bool) -> (String, usize, usize) {
    let mut out: Vec<String> = Vec::new();
    let mut changed = 0usize;
    let mut removed = 0usize;
    let mut seen: HashSet<String> = HashSet::new();
    for raw in text.replace("\r\n", "\n").split('\n') {
        if raw.trim().is_empty() || is_comment(raw) {
            out.push(raw.to_string());
            continue;
        }
        let cleaned = if ip_mode {
            clean_ip_token(raw)
        } else {
            clean_host_token(raw)
        };
        if cleaned.is_empty() {
            removed += 1;
            continue;
        }
        if !seen.insert(cleaned.to_lowercase()) {
            removed += 1;
            continue;
        }
        if cleaned != raw.trim() {
            changed += 1;
        }
        out.push(cleaned);
    }
    (out.join("\n"), changed, removed)
}

// ----------------------------------------------------------------------
// ListChanges
// ----------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ListChanges {
    pub file_name: String,
    pub file_path: PathBuf,
    pub data_dir: PathBuf,
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub baseline: Vec<String>,
    pub current: Vec<String>,
    pub news: Vec<String>,
    pub has_baseline: bool,
    pub last_error: String,
}

pub fn parse_entries(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for raw in lines {
        let t = raw.trim().trim_start_matches('\u{FEFF}');
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        out.push(t.to_string());
    }
    out
}

/// Разделяет тело официального списка и блок добавлений менеджера.
pub fn split_block(lines: &[String]) -> (Vec<String>, Vec<String>) {
    let mut official = Vec::new();
    let mut block = Vec::new();
    let mut in_block = false;
    for raw in lines {
        let line = raw.trim_end_matches(['\r', '\n']).to_string();
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if !in_block && lower.starts_with(&MARK_START_PREFIX.to_lowercase()) {
            in_block = true;
            continue;
        }
        if in_block {
            if lower.starts_with(&MARK_END_PREFIX.to_lowercase()) {
                in_block = false;
                continue;
            }
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                block.push(trimmed.to_string());
            }
            continue;
        }
        official.push(line);
    }
    (official, block)
}

pub fn compose_text(official_lines: &[String], added: &[String]) -> String {
    let mut out = String::new();
    for line in official_lines {
        let lower = line.trim().to_lowercase();
        if lower.starts_with(&MARK_START_PREFIX.to_lowercase())
            || lower.starts_with(&MARK_END_PREFIX.to_lowercase())
        {
            continue;
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    if !added.is_empty() {
        let trimmed = out.trim_end_matches(['\r', '\n']).to_string();
        out = trimmed;
        out.push_str("\r\n\r\n");
        out.push_str(BLOCK_START);
        out.push_str("\r\n");
        for entry in added {
            out.push_str(entry);
            out.push_str("\r\n");
        }
        out.push_str(BLOCK_END);
        out.push_str("\r\n");
    }
    out
}

pub fn contains(list: &[String], value: &str) -> bool {
    list.iter().any(|x| x.eq_ignore_ascii_case(value))
}

pub fn add_unique(list: &mut Vec<String>, value: &str) {
    let v = value.trim();
    if v.is_empty() {
        return;
    }
    if !contains(list, v) {
        list.push(v.to_string());
    }
}

fn baseline_path(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join("lists").join(format!("{}.baseline", name))
}

fn changes_path(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join("lists").join(format!("{}.changes", name))
}

fn news_path(data_dir: &Path, name: &str) -> PathBuf {
    data_dir.join("lists").join(format!("{}.news", name))
}

fn read_changes(data_dir: &Path, name: &str) -> (Vec<String>, Vec<String>) {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for raw in textfile::read_lines(&changes_path(data_dir, name)) {
        let line = raw.trim();
        if line.len() < 2 {
            continue;
        }
        let mut chars = line.chars();
        match chars.next() {
            Some('+') => add_unique(&mut added, chars.as_str()),
            Some('-') => add_unique(&mut removed, chars.as_str()),
            _ => {}
        }
    }
    (added, removed)
}

impl ListChanges {
    pub fn load(file_path: &Path, data_dir: &Path) -> ListChanges {
        let file_name = file_path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut lc = ListChanges {
            file_name: file_name.clone(),
            file_path: file_path.to_path_buf(),
            data_dir: data_dir.to_path_buf(),
            ..Default::default()
        };
        let lines: Vec<String> = textfile::read(file_path)
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .split('\n')
            .map(|s| s.to_string())
            .collect();
        let (official, block) = split_block(&lines);
        lc.current = parse_entries(&official);
        lc.added = block;

        let base_file = baseline_path(data_dir, &file_name);
        if base_file.is_file() {
            lc.baseline = parse_entries(&textfile::read_lines(&base_file));
            lc.has_baseline = true;
        }
        let (extra_added, removed) = read_changes(data_dir, &file_name);
        for a in extra_added {
            add_unique(&mut lc.added, &a);
        }
        lc.removed = removed;

        if lc.has_baseline {
            for entry in lc.current.clone() {
                if !contains(&lc.baseline, &entry) {
                    add_unique(&mut lc.added, &entry);
                }
            }
            for entry in lc.baseline.clone() {
                if !contains(&lc.current, &entry) && !contains(&lc.added, &entry) {
                    add_unique(&mut lc.removed, &entry);
                }
            }
        } else {
            lc.baseline = lc.current.clone();
            lc.has_baseline = true;
            let _ = lc.save_baseline();
        }

        lc.news = parse_entries(&textfile::read_lines(&news_path(data_dir, &file_name)));

        lc.added.retain(|e| !contains(&lc.baseline, e));
        let mut seen = HashSet::new();
        lc.added.retain(|e| seen.insert(e.to_lowercase()));
        lc.removed.retain(|e| contains(&lc.baseline, e));
        let mut seen = HashSet::new();
        lc.removed.retain(|e| seen.insert(e.to_lowercase()));
        lc
    }

    pub fn save_baseline(&self) -> std::io::Result<()> {
        let text = format!("{}\r\n", self.baseline.join("\r\n"));
        textfile::write_text_atomic(&baseline_path(&self.data_dir, &self.file_name), &text)
    }

    pub fn save_changes_file(&self) -> std::io::Result<()> {
        let mut text = String::new();
        text.push_str(&format!(
            "# Изменения пользователя в официальном списке {}\r\n",
            self.file_name
        ));
        text.push_str("# + добавленные строки, - удалённые. Файл ведёт Zapret Manager.\r\n");
        for entry in &self.added {
            text.push_str(&format!("+ {}\r\n", entry));
        }
        for entry in &self.removed {
            text.push_str(&format!("- {}\r\n", entry));
        }
        textfile::write_text_atomic(&changes_path(&self.data_dir, &self.file_name), &text)
    }

    pub fn save_from_editor(&mut self, editor_text: &str) -> Option<String> {
        let lines: Vec<String> = editor_text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .split('\n')
            .map(|s| s.to_string())
            .collect();
        let (official, block) = split_block(&lines);
        let as_read = parse_entries(&official);
        let mut added = block;
        for entry in &as_read {
            if !contains(&self.baseline, entry) {
                add_unique(&mut added, entry);
            }
        }
        let mut removed = Vec::new();
        for entry in &self.baseline {
            if !contains(&as_read, entry) && !contains(&added, entry) {
                add_unique(&mut removed, entry);
            }
        }
        let final_added: Vec<String> = added
            .into_iter()
            .filter(|e| !contains(&self.baseline, e))
            .collect();
        let mut current = as_read.clone();
        for entry in &final_added {
            add_unique(&mut current, entry);
        }

        let mut body = Vec::new();
        for line in &official {
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                body.push(line.clone());
            } else if contains(&self.baseline, line.trim()) {
                body.push(line.clone());
            }
        }
        let text = compose_text(&body, &final_added);
        if let Err(e) = textfile::write_text(&self.file_path, &text) {
            self.last_error = e.to_string();
            return None;
        }
        self.added = final_added;
        self.removed = removed;
        self.current = current;
        if let Err(e) = self.save_changes_file() {
            self.last_error = e.to_string();
            return None;
        }
        Some(text)
    }

    pub fn restore_removed(&mut self, editor_text: &str) -> String {
        let lines: Vec<String> = editor_text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .split('\n')
            .map(|s| s.to_string())
            .collect();
        let (official, block) = split_block(&lines);
        let mut body = Vec::new();
        let present: Vec<String> = parse_entries(&official);
        for line in &official {
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                body.push(line.clone());
            } else if contains(&self.baseline, line.trim()) {
                body.push(line.clone());
            }
        }
        for entry in &self.baseline {
            if !contains(&present, entry) {
                body.push(entry.clone());
            }
        }
        self.removed.clear();
        let mut added = block;
        for entry in &present {
            if !contains(&self.baseline, entry) {
                add_unique(&mut added, entry);
            }
        }
        added.retain(|e| !contains(&self.baseline, e));
        let text = compose_text(&body, &added);
        self.added = added.clone();
        let _ = textfile::write_text(&self.file_path, &text);
        let _ = self.save_changes_file();
        text
    }

    pub fn accept_as_official(&mut self, editor_text: &str) {
        let lines: Vec<String> = editor_text
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .split('\n')
            .map(|s| s.to_string())
            .collect();
        let (official, block) = split_block(&lines);
        let mut all = parse_entries(&official);
        for entry in &block {
            add_unique(&mut all, entry);
        }
        let mut body = official.clone();
        for entry in &block {
            if !contains(&parse_entries(&official), entry) {
                body.push(entry.clone());
            }
        }
        self.baseline = all;
        self.added.clear();
        self.removed.clear();
        let _ = self.save_baseline();
        let _ = self.save_changes_file();
        let text = compose_text(&body, &[]);
        if let Err(e) = textfile::write_text(&self.file_path, &text) {
            self.last_error = e.to_string();
        }
    }

    /// -1 — ошибка, иначе число перенесённых строк.
    pub fn move_added_to_user_list(&mut self, user_list_path: &Path) -> i32 {
        if self.added.is_empty() {
            return 0;
        }
        let existing = parse_entries(&textfile::read_lines(user_list_path));
        let mut to_write = Vec::new();
        for entry in &self.added {
            if !contains(&existing, entry) {
                to_write.push(entry.clone());
            }
        }
        if !to_write.is_empty() {
            let mut text = String::new();
            if existing.is_empty() {
                text.push_str("# Никогда не оставляйте этот файл пустым\r\n");
            }
            for entry in &existing {
                text.push_str(entry);
                text.push_str("\r\n");
            }
            for entry in &to_write {
                text.push_str(entry);
                text.push_str("\r\n");
            }
            if let Err(e) = textfile::write_text_atomic(user_list_path, &text) {
                self.last_error = e.to_string();
                return -1;
            }
        }
        let _ = self.save_changes_file();
        to_write.len() as i32
    }
}

/// Применить сохранённые изменения пользователя к новым официальным спискам.
pub fn apply_after_update(install_root: &Path, data_dir: &Path) -> Vec<String> {
    let mut messages = Vec::new();
    let lists_dir = install_root.join("lists");
    for name in OFFICIAL_FILES {
        let file = lists_dir.join(name);
        if !file.is_file() {
            continue;
        }
        let new_lines: Vec<String> = textfile::read(&file)
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .split('\n')
            .map(|s| s.to_string())
            .collect();
        let new_official = parse_entries(&new_lines);
        let old_baseline = parse_entries(&textfile::read_lines(&baseline_path(data_dir, name)));
        let news: Vec<String> = if old_baseline.is_empty() {
            Vec::new()
        } else {
            new_official
                .iter()
                .filter(|e| !contains(&old_baseline, e))
                .cloned()
                .collect()
        };
        let (added, removed) = read_changes(data_dir, name);
        let mut kept = Vec::new();
        for line in &new_lines {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                kept.push(line.clone());
            } else if !contains(&removed, trimmed) {
                kept.push(line.clone());
            }
        }
        let mut valid_added = Vec::new();
        for entry in &added {
            if !contains(&new_official, entry) {
                add_unique(&mut valid_added, entry);
            }
        }
        let text = compose_text(&kept, &valid_added);
        let _ = textfile::write_text_atomic(&file, &text);

        let baseline_text = format!("{}\r\n", new_official.join("\r\n"));
        let _ = textfile::write_text_atomic(&baseline_path(data_dir, name), &baseline_text);

        let mut lc = ListChanges {
            file_name: name.to_string(),
            data_dir: data_dir.to_path_buf(),
            added: valid_added.clone(),
            removed: removed.clone(),
            ..Default::default()
        };
        let _ = lc.save_changes_file();

        let news_text = news.join("\r\n");
        let _ = textfile::write_text_atomic(&news_path(data_dir, name), &news_text);

        if !valid_added.is_empty() || !removed.is_empty() || !news.is_empty() {
            messages.push(format!(
                "{}: сохранены ваши изменения (+{} / −{}), новых строк от разработчика: {}",
                name,
                valid_added.len(),
                removed.len(),
                news.len()
            ));
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_and_wildcards() {
        assert_eq!(normalize("  Example.COM "), "example.com");
        assert_eq!(normalize("# comment"), "");
        assert_eq!(normalize(";x"), "");
        let (text, fixed) = fix_wildcards("a.com\r\n  *.b.com\r\n*.");
        assert_eq!(fixed, 1);
        assert!(text.contains("  b.com"));
        assert!(text.ends_with("*."));
    }

    #[test]
    fn duplicates() {
        let (text, removed) = remove_duplicates("a.com\nA.com\n# x\nb.com");
        assert_eq!(removed, 1);
        assert_eq!(text, "a.com\n# x\nb.com");
    }

    #[test]
    fn batch() {
        let (items, skipped) = parse_batch(
            "https://Example.com/path, *.foo.bar; https://x.com:8443/a 10.0.0.1/8 ^exact.org",
            false,
        );
        assert_eq!(
            items,
            vec!["example.com", "foo.bar", "x.com", "10.0.0.1", "^exact.org"]
        );
        assert_eq!(skipped, 0);
    }

    #[test]
    fn block_roundtrip() {
        let official = vec!["# official".to_string(), "a.com".to_string()];
        let added = vec!["b.com".to_string()];
        let text = compose_text(&official, &added);
        let lines: Vec<String> = text.split("\r\n").map(|s| s.to_string()).collect();
        let (off, block) = split_block(&lines);
        assert!(off.iter().any(|l| l == "# official"));
        assert!(off.iter().any(|l| l == "a.com"));
        assert!(!off.iter().any(|l| l == "b.com"));
        assert_eq!(block, vec!["b.com"]);
    }
}
