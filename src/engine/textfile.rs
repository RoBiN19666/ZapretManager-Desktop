//! Чтение текстовых файлов с автоопределением UTF-8 / UTF-16 / CP1251.

use std::fs;
use std::path::Path;

use windows_sys::Win32::Globalization::MultiByteToWideChar;

fn decode_utf16_le(bytes: &[u8]) -> String {
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        units.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    String::from_utf16_lossy(&units)
}

fn decode_utf16_be(bytes: &[u8]) -> String {
    let mut units: Vec<u16> = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        units.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    String::from_utf16_lossy(&units)
}

/// CP1251 → UTF-8 через системную функцию Windows.
pub fn decode_cp1251(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    unsafe {
        let needed = MultiByteToWideChar(1251, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0);
        if needed <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        let mut buf: Vec<u16> = vec![0; needed as usize];
        let written = MultiByteToWideChar(
            1251,
            0,
            bytes.as_ptr(),
            bytes.len() as i32,
            buf.as_mut_ptr(),
            needed,
        );
        if written <= 0 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        buf.truncate(written as usize);
        String::from_utf16_lossy(&buf)
    }
}

/// Декодирует байты с учётом BOM; при отсутствии BOM пробует UTF-8, затем CP1251.
pub fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        return decode_utf16_le(&bytes[2..]);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return decode_utf16_be(&bytes[2..]);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => decode_cp1251(bytes),
    }
}

/// Читает текстовый файл целиком. При ошибке — пустая строка (как в C#-версии).
pub fn read(path: &Path) -> String {
    match fs::read(path) {
        Ok(bytes) => decode_bytes(&bytes),
        Err(_) => String::new(),
    }
}

/// Читает файл построчно, нормализуя переводы строк.
pub fn read_lines(path: &Path) -> Vec<String> {
    read(path)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|s| s.to_string())
        .collect()
}

/// Атомарная запись текста: временный файл в том же каталоге, затем переименование.
pub fn write_text_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".to_string());
    let tmp = path.with_file_name(format!(
        ".{}.tmp-{}",
        file_name,
        std::process::id() as u64 ^ crate::util::unix_now()
    ));
    fs::write(&tmp, text.as_bytes())?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Запись UTF-8 без BOM.
pub fn write_text(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            fs::create_dir_all(dir)?;
        }
    }
    fs::write(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_bom() {
        let bytes = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert_eq!(decode_bytes(&bytes), "hi");
    }

    #[test]
    fn utf16() {
        let mut bytes = vec![0xFF, 0xFE];
        for u in "привет".encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        assert_eq!(decode_bytes(&bytes), "привет");
    }

    #[test]
    fn cp1251() {
        // "Тест" в CP1251
        let bytes = [0xD2, 0xE5, 0xF1, 0xF2];
        assert_eq!(decode_cp1251(&bytes), "Тест");
    }
}
