//! Обёртки над Win32 API: окна, реестр, процессы, оболочка, права администратора.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM,
};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_DWORD, REG_SZ,
};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, ShellExecuteW, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetCursorPos, GetForegroundWindow, MessageBoxW, SetForegroundWindow, ShowWindow,
    MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, SW_RESTORE,
    SW_SHOWNORMAL,
};

use crate::util::LocalDateTime;

#[link(name = "ntdll")]
extern "system" {
    fn NtQueryInformationProcess(
        process_handle: HANDLE,
        process_information_class: u32,
        process_information: *mut core::ffi::c_void,
        process_information_length: u32,
        return_length: *mut u32,
    ) -> i32;
}

/// Строка в UTF-16 с завершающим нулём.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Строка из UTF-16 буфера.
pub fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

pub fn last_error() -> u32 {
    unsafe { GetLastError() }
}

/// Текст системной ошибки по коду.
pub fn error_text(code: u32) -> String {
    std::io::Error::from_raw_os_error(code as i32).to_string()
}

// ---------------------------------------------------------------------------
// Права администратора
// ---------------------------------------------------------------------------

/// Запущен ли процесс с повышенными правами (UAC).
pub fn is_admin() -> bool {
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut size: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut core::ffi::c_void,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        );
        CloseHandle(token);
        ok != 0 && elevation.TokenIsElevated != 0
    }
}

// ---------------------------------------------------------------------------
// Оболочка: сообщения, ссылки, папки, запуск от администратора
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MsgKind {
    Info,
    Warn,
    Error,
}

pub fn message_box(text: &str, title: &str, kind: MsgKind) {
    let icon = match kind {
        MsgKind::Info => MB_ICONINFORMATION,
        MsgKind::Warn => MB_ICONWARNING,
        MsgKind::Error => MB_ICONERROR,
    };
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(text).as_ptr(),
            wide(title).as_ptr(),
            MB_OK | icon | MB_SETFOREGROUND,
        );
    }
}

fn shell_execute(verb: &str, file: &str, params: Option<&str>, cwd: Option<&str>) -> bool {
    let verb_w = wide(verb);
    let file_w = wide(file);
    let params_w = params.map(wide);
    let cwd_w = cwd.map(wide);
    unsafe {
        let r = ShellExecuteW(
            std::ptr::null_mut(),
            verb_w.as_ptr(),
            file_w.as_ptr(),
            params_w.as_ref().map(|v| v.as_ptr()).unwrap_or(std::ptr::null()),
            cwd_w.as_ref().map(|v| v.as_ptr()).unwrap_or(std::ptr::null()),
            SW_SHOWNORMAL,
        );
        r as isize > 32
    }
}

/// Открыть папку в проводнике.
pub fn open_folder(path: &Path) {
    if path.is_dir() {
        let _ = shell_execute("open", &path.to_string_lossy(), None, None);
    }
}

/// Показать файл в проводнике (с выделением).
pub fn select_in_explorer(file: &Path) {
    if file.exists() {
        let parent = file.parent().map(|p| p.to_path_buf());
        let _ = shell_execute(
            "open",
            "explorer.exe",
            Some(&format!("/select,\"{}\"", file.to_string_lossy())),
            parent.map(|p| p.to_string_lossy().to_string()).as_deref(),
        );
    } else if let Some(parent) = file.parent() {
        open_folder(parent);
    }
}

/// Открыть ссылку в браузере по умолчанию.
pub fn open_url(url: &str) {
    let _ = shell_execute("open", url, None, None);
}

/// Перезапустить текущий exe с правами администратора.
pub fn elevate_self(args: &[String]) -> bool {
    let exe = std::env::current_exe().unwrap_or_default();
    let params = args.join(" ");
    let verb = wide("runas");
    let file = wide(&exe.to_string_lossy());
    let params_w = wide(&params);
    let cwd = exe
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let cwd_w = wide(&cwd);
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = 0x0000_0040; // SEE_MASK_NOCLOSEPROCESS
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params_w.as_ptr();
    info.lpDirectory = cwd_w.as_ptr();
    info.nShow = SW_SHOWNORMAL;
    unsafe {
        let ok = ShellExecuteExW(&mut info) != 0;
        if !info.hProcess.is_null() {
            CloseHandle(info.hProcess);
        }
        ok
    }
}

/// Активировать уже запущенное окно по заголовку.
pub fn activate_window(title: &str) -> bool {
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), wide(title).as_ptr());
        if hwnd.is_null() {
            return false;
        }
        ShowWindow(hwnd, SW_RESTORE);
        SetForegroundWindow(hwnd);
        true
    }
}

pub fn window_exists(title: &str) -> bool {
    unsafe { !FindWindowW(std::ptr::null(), wide(title).as_ptr()).is_null() }
}

/// Активировать произвольное окно по HWND.
pub fn focus_window(hwnd: HWND) {
    unsafe {
        if GetForegroundWindow() != hwnd {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }
}

/// Тёмный/светлый заголовок окна (Windows 10 1809+).
pub fn set_titlebar_theme(hwnd: HWND, dark: bool) {
    if hwnd.is_null() {
        return;
    }
    let value: i32 = if dark { 1 } else { 0 };
    unsafe {
        let hr = DwmSetWindowAttribute(
            hwnd,
            20, // DWMWA_USE_IMMERSIVE_DARK_MODE
            &value as *const i32 as *const core::ffi::c_void,
            4,
        );
        if hr != 0 {
            let _ = DwmSetWindowAttribute(
                hwnd,
                19,
                &value as *const i32 as *const core::ffi::c_void,
                4,
            );
        }
    }
}

pub fn cursor_pos() -> (i32, i32) {
    let mut p = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

// ---------------------------------------------------------------------------
// Время
// ---------------------------------------------------------------------------

pub fn local_now() -> LocalDateTime {
    let mut st: windows_sys::Win32::Foundation::SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut st) };
    LocalDateTime {
        year: st.wYear,
        month: st.wMonth,
        day: st.wDay,
        hour: st.wHour,
        minute: st.wMinute,
        second: st.wSecond,
    }
}

// ---------------------------------------------------------------------------
// Реестр
// ---------------------------------------------------------------------------

pub struct RegKey(HKEY);

impl Drop for RegKey {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

impl RegKey {
    pub fn open(machine: bool, path: &str, write: bool) -> Option<RegKey> {
        let root = if machine { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
        let mut key: HKEY = std::ptr::null_mut();
        let access = if write { KEY_READ | KEY_WRITE } else { KEY_READ };
        let rc = unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, access, &mut key) };
        if rc != 0 {
            None
        } else {
            Some(RegKey(key))
        }
    }

    pub fn create(machine: bool, path: &str) -> Option<RegKey> {
        let root = if machine { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
        let mut key: HKEY = std::ptr::null_mut();
        let mut disposition = 0u32;
        let rc = unsafe {
            RegCreateKeyExW(
                root,
                wide(path).as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_READ | KEY_WRITE,
                std::ptr::null(),
                &mut key,
                &mut disposition,
            )
        };
        if rc != 0 {
            None
        } else {
            Some(RegKey(key))
        }
    }

    pub fn get_string(&self, name: &str) -> Option<String> {
        let mut kind = 0u32;
        let mut size = 0u32;
        let rc = unsafe {
            RegQueryValueExW(
                self.0,
                wide(name).as_ptr(),
                std::ptr::null(),
                &mut kind,
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if rc != 0 || size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize + 2];
        let rc = unsafe {
            RegQueryValueExW(
                self.0,
                wide(name).as_ptr(),
                std::ptr::null(),
                &mut kind,
                buf.as_mut_ptr(),
                &mut size,
            )
        };
        if rc != 0 {
            return None;
        }
        let units: Vec<u16> = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Some(from_wide(&units))
    }

    pub fn get_dword(&self, name: &str) -> Option<u32> {
        let mut kind = 0u32;
        let mut data: u32 = 0;
        let mut size: u32 = 4;
        let rc = unsafe {
            RegQueryValueExW(
                self.0,
                wide(name).as_ptr(),
                std::ptr::null(),
                &mut kind,
                &mut data as *mut u32 as *mut u8,
                &mut size,
            )
        };
        if rc != 0 {
            None
        } else {
            Some(data)
        }
    }

    pub fn set_string(&self, name: &str, value: &str) -> bool {
        let data = wide(value);
        let bytes = (data.len() - 1) * 2;
        let rc = unsafe {
            RegSetValueExW(
                self.0,
                wide(name).as_ptr(),
                0,
                REG_SZ,
                data.as_ptr() as *const u8,
                bytes as u32,
            )
        };
        rc == 0
    }

    pub fn set_dword(&self, name: &str, value: u32) -> bool {
        let rc = unsafe {
            RegSetValueExW(
                self.0,
                wide(name).as_ptr(),
                0,
                REG_DWORD,
                &value as *const u32 as *const u8,
                4,
            )
        };
        rc == 0
    }

    pub fn delete_value(&self, name: &str) -> bool {
        unsafe { RegDeleteValueW(self.0, wide(name).as_ptr()) == 0 }
    }
}

pub fn reg_get_string(machine: bool, path: &str, name: &str) -> Option<String> {
    RegKey::open(machine, path, false).and_then(|k| k.get_string(name))
}

pub fn reg_set_string(machine: bool, path: &str, name: &str, value: &str) -> bool {
    RegKey::create(machine, path)
        .map(|k| k.set_string(name, value))
        .unwrap_or(false)
}

pub fn reg_set_dword(machine: bool, path: &str, name: &str, value: u32) -> bool {
    RegKey::create(machine, path)
        .map(|k| k.set_dword(name, value))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Процессы
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub exe: PathBuf,
    pub command_line: String,
    pub started_unix: Option<u64>,
}

/// Список процессов через Toolhelp (без WMI).
pub fn list_processes() -> Vec<ProcInfo> {
    let mut list = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap as isize == -1 {
            return list;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let name = from_wide(&entry.szExeFile);
                list.push(ProcInfo {
                    pid: entry.th32ProcessID,
                    parent_pid: entry.th32ParentProcessID,
                    name,
                    ..Default::default()
                });
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
    }
    list
}

/// Командная строка процесса через NtQueryInformationProcess(ProcessCommandLineInformation).
pub fn process_command_line(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut size: u32 = 0;
        NtQueryInformationProcess(
            handle,
            60,
            std::ptr::null_mut(),
            0,
            &mut size,
        );
        let mut buf = vec![0u8; size.max(64) as usize + 32];
        let status = NtQueryInformationProcess(
            handle,
            60,
            buf.as_mut_ptr() as *mut core::ffi::c_void,
            buf.len() as u32,
            &mut size,
        );
        CloseHandle(handle);
        if status < 0 {
            return None;
        }
        // UNICODE_STRING: Length(2) MaximumLength(2) padding(4) Buffer(8)
        let length = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        let ptr = usize::from_le_bytes([
            buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
        ]) as *const u16;
        if ptr.is_null() || length == 0 {
            return None;
        }
        let units = std::slice::from_raw_parts(ptr, length / 2);
        Some(String::from_utf16_lossy(units))
    }
}

/// Полный путь к exe процесса.
pub fn process_exe_path(pid: u32) -> Option<PathBuf> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 32768];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..size as usize])))
    }
}

/// Время старта процесса (секунды Unix).
pub fn process_start_unix(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetProcessTimes, GetCurrentProcess};
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        let handle = if handle.is_null() {
            GetCurrentProcess()
        } else {
            handle
        };
        let mut creation: FILETIME = std::mem::zeroed();
        let mut exit: FILETIME = std::mem::zeroed();
        let mut kernel: FILETIME = std::mem::zeroed();
        let mut user: FILETIME = std::mem::zeroed();
        let ok = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        if handle != GetCurrentProcess() {
            CloseHandle(handle);
        }
        if ok == 0 {
            return None;
        }
        let ticks = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
        // 100-нс интервалы с 1601-01-01; переводим в секунды Unix.
        Some(ticks / 10_000_000 - 11_644_473_600)
    }
}

pub fn process_exit_code(pid: u32) -> Option<u32> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        if ok == 0 {
            None
        } else {
            Some(code)
        }
    }
}

pub fn kill_process(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(
            windows_sys::Win32::System::Threading::PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        );
        if handle.is_null() {
            return false;
        }
        let ok = TerminateProcess(handle, 1);
        CloseHandle(handle);
        ok != 0
    }
}

// ---------------------------------------------------------------------------
// Один экземпляр приложения
// ---------------------------------------------------------------------------

struct SendHandle(HANDLE);
unsafe impl Send for SendHandle {}

static INSTANCE_MUTEX: Mutex<Option<SendHandle>> = Mutex::new(None);

/// Захватить именованный мьютекс. `false` — экземпляр уже запущен.
pub fn acquire_single_instance(name: &str) -> bool {
    use windows_sys::Win32::System::Threading::CreateMutexW;
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 1, wide(name).as_ptr());
        if handle.is_null() {
            return true;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return false;
        }
        if let Ok(mut guard) = INSTANCE_MUTEX.lock() {
            *guard = Some(SendHandle(handle));
        }
        true
    }
}

/// Похоже ли, что окно свёрнуто.
pub fn is_minimized(hwnd: HWND) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsIconic;
    unsafe { IsIconic(hwnd) != 0 }
}

/// Заголовок окна по HWND (для отладки и снимков).
pub fn window_title(hwnd: HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        from_wide(&buf)
    }
}

/// Видимые окна верхнего уровня: (PID, заголовок).
pub fn visible_windows() -> Vec<(u32, String)> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };

    static COLLECT: Mutex<Vec<(u32, String)>> = Mutex::new(Vec::new());

    unsafe extern "system" fn callback(hwnd: HWND, _lparam: LPARAM) -> i32 {
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return 1;
        }
        let title = window_title(hwnd);
        if !title.trim().is_empty() {
            if let Ok(mut list) = COLLECT.lock() {
                list.push((pid, title));
            }
        }
        1
    }

    if let Ok(mut list) = COLLECT.lock() {
        list.clear();
    }
    unsafe {
        EnumWindows(Some(callback), 0);
    }
    COLLECT.lock().map(|l| l.clone()).unwrap_or_default()
}

/// Подключи недавно созданные окна? Не требуется: заголовок читается из HWND напрямую.
pub fn window_handle_by_title(title: &str) -> HWND {
    unsafe { FindWindowW(std::ptr::null(), wide(title).as_ptr()) }
}

/// Перечислить подключи реестра.
pub fn enum_subkeys(machine: bool, path: &str) -> Vec<String> {
    use windows_sys::Win32::System::Registry::{RegEnumKeyExW, HKEY_CURRENT_USER};
    let root = if machine { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
    let mut key: HKEY = std::ptr::null_mut();
    let rc = unsafe { RegOpenKeyExW(root, wide(path).as_ptr(), 0, KEY_READ, &mut key) };
    if rc != 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut index = 0u32;
    loop {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        let rc = unsafe {
            RegEnumKeyExW(
                key,
                index,
                name.as_mut_ptr(),
                &mut len,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc != 0 {
            break;
        }
        out.push(from_wide(&name[..len as usize]));
        index += 1;
    }
    unsafe {
        RegCloseKey(key);
    }
    out
}

/// Константа HWND для передачи в UI-слой.
pub const NULL_HWND: HWND = std::ptr::null_mut();

// LPARAM используется в tray-модуле, держим импорт живым.
#[allow(dead_code)]
pub(crate) fn lparam(x: isize) -> LPARAM {
    x as LPARAM
}
