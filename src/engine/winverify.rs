//! Проверка Authenticode-подписи PE-файла через `WinVerifyTrust`.
//!
//! Нужна самообновлению: перед заменой exe важно убедиться, что скачанный
//! файл либо подписан доверенным сертификатом, либо хотя бы совпал по
//! контрольной сумме с тем, что опубликован в релизе. Проверка идёт без
//! обращения к сети (`WTD_CACHE_ONLY_URL_RETRIEVAL`), чтобы не зависать на
//! недоступных CRL/OCSP.

use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureState {
    /// Подписи нет (обычный случай для неподписанной сборки).
    Unsigned,
    /// Подпись есть и цепочка сертификатов доверенная.
    Valid,
    /// Подпись есть, но недействительна: код возврата `WinVerifyTrust`.
    Invalid(i32),
}

impl SignatureState {
    pub fn is_valid(self) -> bool {
        matches!(self, SignatureState::Valid)
    }

    pub fn text(self) -> String {
        match self {
            SignatureState::Unsigned => "без подписи".to_string(),
            SignatureState::Valid => "подпись действительна".to_string(),
            SignatureState::Invalid(code) => {
                format!("подпись недействительна (0x{:08X})", code as u32)
            }
        }
    }
}

// HRESULT-ы, которые означают именно «подписи нет», а не «подпись плохая».
const TRUST_E_PROVIDER_UNKNOWN: i32 = 0x800B_0001u32 as i32;
const TRUST_E_SUBJECT_FORM_UNKNOWN: i32 = 0x800B_0003u32 as i32;
const TRUST_E_NOSIGNATURE: i32 = 0x800B_0100u32 as i32;

#[cfg(windows)]
pub fn check(path: &Path) -> SignatureState {
    map_hr(verify_raw(path))
}

/// Код возврата `WinVerifyTrust`. Отдельно от `check`, чтобы отображение
/// HRESULT-ов можно было протестировать без Windows.
#[cfg(windows)]
fn verify_raw(path: &Path) -> i32 {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Security::WinTrust::{
        WinVerifyTrust, WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_FILE_INFO,
        WTD_CACHE_ONLY_URL_RETRIEVAL, WTD_CHOICE_FILE, WTD_REVOKE_WHOLECHAIN, WTD_STATEACTION_CLOSE,
        WTD_STATEACTION_VERIFY, WTD_UI_NONE,
    };

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: структуры Win32 заполняются целиком (нулевая инициализация
    // соответствует «не используется»), указатель на путь живёт до конца
    // вызова, `cbStruct` выставлен по фактическому размеру.
    unsafe {
        let mut file_info: WINTRUST_FILE_INFO = std::mem::zeroed();
        file_info.cbStruct = std::mem::size_of::<WINTRUST_FILE_INFO>() as u32;
        file_info.pcwszFilePath = wide.as_ptr();

        let mut data: WINTRUST_DATA = std::mem::zeroed();
        data.cbStruct = std::mem::size_of::<WINTRUST_DATA>() as u32;
        data.dwUIChoice = WTD_UI_NONE;
        // Отзыв проверяем по всей цепочке. `WTD_CACHE_ONLY_URL_RETRIEVAL` ниже
        // оставляет проверку офлайновой: используются только уже закэшированные
        // CRL/OCSP, без зависания на недоступных серверах.
        data.fdwRevocationChecks = WTD_REVOKE_WHOLECHAIN;
        data.dwUnionChoice = WTD_CHOICE_FILE;
        data.Anonymous.pFile = &mut file_info;
        data.dwStateAction = WTD_STATEACTION_VERIFY;
        data.dwProvFlags = WTD_CACHE_ONLY_URL_RETRIEVAL;

        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let hwnd: _ = std::mem::zeroed();
        let status = WinVerifyTrust(
            hwnd,
            &mut action,
            &mut data as *mut WINTRUST_DATA as *mut c_void,
        );

        // Освобождаем состояние, иначе WinTrust держит дескриптор файла.
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        let _ = WinVerifyTrust(
            hwnd,
            &mut action,
            &mut data as *mut WINTRUST_DATA as *mut c_void,
        );

        status
    }
}

fn map_hr(status: i32) -> SignatureState {
    match status {
        0 => SignatureState::Valid,
        TRUST_E_NOSIGNATURE | TRUST_E_SUBJECT_FORM_UNKNOWN | TRUST_E_PROVIDER_UNKNOWN => {
            SignatureState::Unsigned
        }
        other => SignatureState::Invalid(other),
    }
}

#[cfg(not(windows))]
pub fn check(_path: &Path) -> SignatureState {
    SignatureState::Unsigned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hresult_mapping() {
        assert_eq!(map_hr(0), SignatureState::Valid);
        assert_eq!(map_hr(0x800B_0100u32 as i32), SignatureState::Unsigned);
        assert_eq!(map_hr(0x800B_0003u32 as i32), SignatureState::Unsigned);
        assert_eq!(map_hr(0x800B_0001u32 as i32), SignatureState::Unsigned);
        // TRUST_E_BAD_DIGEST — файл подписан, но содержимое изменено.
        assert!(matches!(
            map_hr(0x8009_6010u32 as i32),
            SignatureState::Invalid(_)
        ));
    }

    #[test]
    fn missing_file_is_not_valid() {
        let state = check(Path::new(r"C:\Windows\System32\__no_such_file__.exe"));
        assert!(!state.is_valid());
    }

    /// Неподписанный exe (тестовый бинарник cargo) не должен считаться
    /// подписанным и не должен ронять проверку.
    #[cfg(windows)]
    #[test]
    fn unsigned_build_is_unsigned() {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        assert_eq!(check(&exe), SignatureState::Unsigned);
    }
}
