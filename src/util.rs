//! Разные утилиты: сортировка, версии, размеры, время, SHA-256, валидация доменов.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Сортировка «как в проводнике»: ALT2 < ALT10
// ---------------------------------------------------------------------------

/// Ключ натуральной сортировки: числа внутри сравниваются как числа.
fn natural_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut num = String::new();
            num.push(c);
            while let Some(&n) = chars.peek() {
                if n.is_ascii_digit() {
                    num.push(n);
                    chars.next();
                } else {
                    break;
                }
            }
            let value: u64 = num.parse().unwrap_or(0);
            out.push_str(&format!("{:012}", value));
        } else {
            for lower in c.to_lowercase() {
                out.push(lower);
            }
        }
    }
    out
}

/// Натуральное сравнение имён файлов стратегий (регистронезависимое).
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    natural_key(a).cmp(&natural_key(b))
}

// ---------------------------------------------------------------------------
// Версии
// ---------------------------------------------------------------------------

/// Разобранная версия: числовые части + суффикс.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VersionKey {
    pub numbers: Vec<u64>,
    pub suffix: String,
    pub pre_release: bool,
}

/// Тег без префикса v: "v1.10.4" → "1.10.4".
pub fn normalize_tag(tag: &str) -> String {
    let t = tag.trim();
    let t = t.trim_start_matches(['v', 'V']);
    t.to_string()
}

pub fn parse_version(v: &str) -> VersionKey {
    let mut key = VersionKey::default();
    let v = normalize_tag(v);
    let (digits, rest) = match v.find(|c: char| !(c.is_ascii_digit() || c == '.')) {
        Some(i) => (&v[..i], &v[i..]),
        None => (v.as_str(), ""),
    };
    if digits.is_empty() {
        return key;
    }
    for part in digits.split('.') {
        key.numbers.push(part.parse::<u64>().unwrap_or(0));
    }
    let suffix = rest.trim().trim_start_matches(['-', '.', '_', '+']);
    key.suffix = suffix.to_string();
    let lower = suffix.to_ascii_lowercase();
    for prefix in ["pre", "alpha", "beta", "rc", "dev"] {
        if lower.starts_with(prefix) {
            let tail = &lower[prefix.len()..];
            if tail.is_empty() || tail.starts_with(|c: char| c.is_ascii_digit() || !c.is_alphanumeric()) {
                key.pre_release = true;
            }
            break;
        }
    }
    key
}

/// Сравнение версий: 1.4.0-pre1 < 1.4.0, 1.9.9d > 1.9.9.
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let va = parse_version(a);
    let vb = parse_version(b);
    let n = va.numbers.len().max(vb.numbers.len());
    for i in 0..n {
        let x = va.numbers.get(i).copied().unwrap_or(0);
        let y = vb.numbers.get(i).copied().unwrap_or(0);
        if x != y {
            return x.cmp(&y);
        }
    }
    if va.pre_release != vb.pre_release {
        return if va.pre_release {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        };
    }
    // Суффиксы сравниваем по кускам: `beta9` < `beta10`, а не побайтово.
    cmp_version_suffix(&va.suffix, &vb.suffix)
}

/// Сравнение суффиксов версий с учётом чисел внутри.
///
/// `beta2` < `beta10`, `rc1` < `rc2`; при равенстве чисел — побайтовое
/// сравнение текстовых кусков. Цифровые куски считаются меньше буквенных
/// (так `1.0.0-1` < `1.0.0-beta`), что совпадает с ожиданиями semver.
fn cmp_version_suffix(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    fn chunks(s: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let digit = bytes[i].is_ascii_digit();
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() == digit {
                i += 1;
            }
            // Границы всегда на ASCII-байтах, поэтому срез по UTF-8 корректен.
            out.push((digit, &s[start..i]));
        }
        out
    }

    fn num_cmp(x: &str, y: &str) -> Ordering {
        let x = x.trim_start_matches('0');
        let y = y.trim_start_matches('0');
        x.len().cmp(&y.len()).then_with(|| x.cmp(y))
    }

    let ca = chunks(a);
    let cb = chunks(b);
    for i in 0..ca.len().max(cb.len()) {
        match (ca.get(i).copied(), cb.get(i).copied()) {
            (Some((da, sa)), Some((db, sb))) => {
                let ord = match (da, db) {
                    (true, true) => num_cmp(sa, sb),
                    (false, false) => sa.cmp(sb),
                    (true, false) => Ordering::Less,
                    (false, true) => Ordering::Greater,
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => break,
        }
    }
    Ordering::Equal
}

// ---------------------------------------------------------------------------
// Размеры
// ---------------------------------------------------------------------------

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} ГБ", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} МБ", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} КБ", bytes as f64 / KB as f64)
    } else {
        format!("{} Б", bytes)
    }
}

// ---------------------------------------------------------------------------
// Время
// ---------------------------------------------------------------------------

/// Локальная дата/время без внешних зависимостей.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct LocalDateTime {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
}

impl std::fmt::Display for LocalDateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

impl LocalDateTime {
    /// "HH:MM:SS" — как в журнале.
    pub fn time_string(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

/// Секунды с начала эпохи (UTC) для арифметики со временем.
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Смещение локального часового пояса от UTC в секундах.
///
/// Считается по текущему моменту: сравниваем локальные компоненты с UTC.
/// Учитывает действующее сейчас смещение (включая летнее время), но не
/// исторические переходы — для меток времени файлов этого достаточно.
pub fn local_offset_secs() -> i64 {
    let now = unix_now() as i64;
    let utc = datetime_from_secs(now);
    let local = crate::engine::sys::local_now();
    let utc_days = days_from_civil(utc.year as i64, utc.month as i64, utc.day as i64);
    let local_days = days_from_civil(local.year as i64, local.month as i64, local.day as i64);
    let utc_clock = utc.hour as i64 * 3600 + utc.minute as i64 * 60 + utc.second as i64;
    let local_clock = local.hour as i64 * 3600 + local.minute as i64 * 60 + local.second as i64;
    (local_days - utc_days) * 86400 + (local_clock - utc_clock)
}

/// Локальное время из unix-секунд (например, время изменения файла).
pub fn datetime_local_from_secs(secs: i64) -> LocalDateTime {
    datetime_from_secs(secs + local_offset_secs())
}

/// Дни от 1970-01-01 до гражданской даты (алгоритм Хауарда Хиннанта).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Обратное преобразование: дни от эпохи → (год, месяц, день).
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Разбор строки "YYYY-MM-DD HH:MM:SS" (и просто даты) в секунды UTC-подобной шкалы.
pub fn parse_datetime_secs(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split_whitespace().collect();
    let date = parts.first()?;
    let mut dparts = date.split('-');
    let y: i64 = dparts.next()?.parse().ok()?;
    let m: i64 = dparts.next()?.parse().ok()?;
    let d: i64 = dparts.next()?.parse().ok()?;
    let (mut hh, mut mm, mut ss) = (0i64, 0i64, 0i64);
    if let Some(time) = parts.get(1) {
        let mut tparts = time.split(':');
        hh = tparts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        mm = tparts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        ss = tparts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    }
    Some(days_from_civil(y, m, d) * 86400 + hh * 3600 + mm * 60 + ss)
}

/// Локальное время из секунд шкалы UTC (с учётом смещения пояса не возимся:
/// для интервалов «прошло N часов» достаточно одной шкалы).
pub fn datetime_from_secs(secs: i64) -> LocalDateTime {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    LocalDateTime {
        year: y as u16,
        month: m as u16,
        day: d as u16,
        hour: (rem / 3600) as u16,
        minute: ((rem % 3600) / 60) as u16,
        second: (rem % 60) as u16,
    }
}

/// «5 мин 12 с» из секунд.
pub fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        return format!("{} с", secs);
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{} мин {} с", mins, secs % 60);
    }
    let hours = mins / 60;
    format!("{} ч {} мин", hours, mins % 60)
}

// ---------------------------------------------------------------------------
// SHA-256 (чистый Rust, без зависимостей)
// ---------------------------------------------------------------------------

pub struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total: u64,
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    pub fn new() -> Self {
        Sha256 {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0u8; 64],
            buf_len: 0,
            total: 0,
        }
    }

    fn compress(&mut self, block: &[u8]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
        while data.len() >= 64 {
            self.compress(&data[..64]);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    pub fn finish(mut self) -> [u8; 32] {
        let bit_len = self.total.wrapping_mul(8);
        self.update(&[0x80]);
        while self.buf_len != 56 {
            self.update(&[0]);
        }
        self.update(&bit_len.to_be_bytes());
        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finish()
}

pub fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// SHA-256 файла (пустая строка при ошибке).
pub fn sha256_file(path: &std::path::Path) -> String {
    use std::io::Read;
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let mut h = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => h.update(&buf[..n]),
            Err(_) => return String::new(),
        }
    }
    hex_lower(&h.finish())
}

// ---------------------------------------------------------------------------
// Текст
// ---------------------------------------------------------------------------

/// Схлопывает пробелы и обрезает края.
pub fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Обрезает строку до max символов, добавляя многоточие.
pub fn shorten(text: &str, max: usize) -> String {
    let flat = text.replace(['\r', '\n'], " ");
    let flat = flat.trim();
    if flat.chars().count() <= max {
        flat.to_string()
    } else {
        let mut s: String = flat.chars().take(max).collect();
        s.push('…');
        s
    }
}

/// Проверка строки портов вида "1024-65535" или "80,443,1000-2000".
pub fn is_port_range_valid(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    for raw in text.split(',') {
        let p = raw.trim();
        if p.is_empty() {
            return false;
        }
        let mut parts = p.split('-');
        let a = parts.next().unwrap_or("");
        let b = parts.next();
        if parts.next().is_some() {
            return false;
        }
        let parse_port = |s: &str| -> Option<u32> {
            if s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()) || s.starts_with('0') {
                return None;
            }
            let v: u32 = s.parse().ok()?;
            if v == 0 || v > 65535 {
                return None;
            }
            Some(v)
        };
        let pa = match parse_port(a) {
            Some(v) => v,
            None => return false,
        };
        if let Some(b) = b {
            match parse_port(b) {
                Some(pb) if pb >= pa => {}
                _ => return false,
            }
        }
    }
    true
}

/// Допустимое имя домена для списков zapret: буквы/цифры/дефис, точки, необязательная ^ в начале.
pub fn is_valid_host_line(line: &str) -> bool {
    let mut s = line.trim();
    if s.is_empty() {
        return false;
    }
    if let Some(rest) = s.strip_prefix('^') {
        s = rest;
    }
    if s.is_empty() || s.starts_with('.') || s.ends_with('.') || s.contains("..") {
        return false;
    }
    let mut labels = 0;
    for label in s.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        for c in label.chars() {
            if !(c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return false;
            }
        }
        labels += 1;
    }
    labels >= 1 && s.len() <= 253
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec!["general (ALT10)", "general", "general (ALT2)", "general (ALT)"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, vec!["general", "general (ALT)", "general (ALT2)", "general (ALT10)"]);
    }

    #[test]
    fn versions() {
        assert_eq!(version_cmp("1.10.4", "1.9.9"), std::cmp::Ordering::Greater);
        assert_eq!(version_cmp("1.4.0-pre1", "1.4.0"), std::cmp::Ordering::Less);
        assert_eq!(version_cmp("v1.9.9d", "1.9.9"), std::cmp::Ordering::Greater);
        assert_eq!(version_cmp("1.4.0", "1.4.0.0"), std::cmp::Ordering::Equal);
        // Суффиксы с числами сравниваются как числа, а не побайтово.
        assert_eq!(
            version_cmp("1.4.0-beta9", "1.4.0-beta10"),
            std::cmp::Ordering::Less
        );
        assert_eq!(version_cmp("2.0.0-rc1", "2.0.0-rc2"), std::cmp::Ordering::Less);
        assert_eq!(version_cmp("2.0.0-rc", "2.0.0-rc1"), std::cmp::Ordering::Less);
        assert_eq!(version_cmp("2.0.0-beta", "2.0.0-rc"), std::cmp::Ordering::Less);
    }

    #[test]
    fn sha256_known() {
        assert_eq!(
            hex_lower(&sha256_bytes(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let mut h = Sha256::new();
        for chunk in [&b"a"[..], &b"b"[..], &b"c"[..]] {
            h.update(chunk);
        }
        assert_eq!(hex_lower(&h.finish()), hex_lower(&sha256_bytes(b"abc")));
    }

    #[test]
    fn dates_roundtrip() {
        let secs = parse_datetime_secs("2026-11-26 17:17:11").unwrap();
        let dt = datetime_from_secs(secs);
        assert_eq!(dt.year, 2026);
        assert_eq!(dt.month, 11);
        assert_eq!(dt.day, 26);
        assert_eq!(dt.hour, 17);
    }

    #[test]
    fn ranges() {
        assert!(is_port_range_valid("1024-65535"));
        assert!(is_port_range_valid("80,443,1000-2000"));
        assert!(!is_port_range_valid("0-100"));
        assert!(!is_port_range_valid("2000-1000"));
        assert!(!is_port_range_valid("1024-"));
    }

    #[test]
    fn hosts() {
        assert!(is_valid_host_line("example.com"));
        assert!(is_valid_host_line("^example.com"));
        assert!(!is_valid_host_line("*.example.com"));
        assert!(!is_valid_host_line("http://example.com"));
        assert!(!is_valid_host_line("-bad.com"));
    }
}
