//! Управление службой Windows через Service Control Manager (без sc.exe).

use windows_sys::Win32::Foundation::{GetLastError, ERROR_SERVICE_DOES_NOT_EXIST};
use windows_sys::Win32::System::Services::{
    ChangeServiceConfig2W, CloseServiceHandle, ControlService, CreateServiceW, DeleteService,
    OpenSCManagerW, OpenServiceW, QueryServiceStatusEx, StartServiceW, SC_HANDLE,
    SERVICE_DESCRIPTIONW, SERVICE_STATUS_PROCESS,
};

use super::sys::{error_text, wide};

const SC_MANAGER_ALL_ACCESS: u32 = 0xF003F;
const SERVICE_ALL_ACCESS: u32 = 0xF01FF;
const SERVICE_QUERY_CONFIG: u32 = 0x0001;
const SERVICE_QUERY_STATUS: u32 = 0x0004;
const SERVICE_START: u32 = 0x0010;
const SERVICE_STOP: u32 = 0x0020;
const DELETE_ACCESS: u32 = 0x10000;
const SERVICE_WIN32_OWN_PROCESS: u32 = 0x00000010;
const SERVICE_AUTO_START: u32 = 0x00000002;
const SERVICE_ERROR_NORMAL: u32 = 0x00000001;
const SERVICE_CONTROL_STOP: u32 = 0x00000001;
const SERVICE_CONFIG_DESCRIPTION: u32 = 1;
const SC_STATUS_PROCESS_INFO: u32 = 0;

#[derive(Debug, Clone, Default)]
pub struct ServiceSnapshot {
    pub name: String,
    pub installed: bool,
    pub status: String,
    pub strategy: String,
    pub image_path: String,
}

struct ScHandle(SC_HANDLE);

impl Drop for ScHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseServiceHandle(self.0);
            }
        }
    }
}

fn open_scm(create: bool) -> Result<ScHandle, String> {
    let access = if create {
        SC_MANAGER_ALL_ACCESS
    } else {
        0x0001 // SC_MANAGER_CONNECT
    };
    unsafe {
        let scm = OpenSCManagerW(std::ptr::null(), std::ptr::null(), access);
        if scm.is_null() {
            return Err(format!(
                "Не удалось открыть менеджер служб (нужны права администратора): {}",
                error_text(GetLastError())
            ));
        }
        Ok(ScHandle(scm))
    }
}

fn open_service(scm: &ScHandle, name: &str, access: u32) -> Result<ScHandle, u32> {
    unsafe {
        let h = OpenServiceW(scm.0, wide(name).as_ptr(), access);
        if h.is_null() {
            Err(GetLastError())
        } else {
            Ok(ScHandle(h))
        }
    }
}

/// Состояние службы: "работает", "остановлена", "не установлена"…
pub fn status(name: &str) -> String {
    let scm = match open_scm(false) {
        Ok(s) => s,
        Err(_) => return "не установлена".to_string(),
    };
    let svc = match open_service(&scm, name, SERVICE_QUERY_STATUS) {
        Ok(s) => s,
        Err(_) => return "не установлена".to_string(),
    };
    let mut status: SERVICE_STATUS_PROCESS = unsafe { std::mem::zeroed() };
    let mut needed: u32 = 0;
    let ok = unsafe {
        QueryServiceStatusEx(
            svc.0,
            0, // SC_STATUS_PROCESS_INFO
            &mut status as *mut SERVICE_STATUS_PROCESS as *mut u8,
            std::mem::size_of::<SERVICE_STATUS_PROCESS>() as u32,
            &mut needed,
        )
    };
    if ok == 0 {
        return "не установлена".to_string();
    }
    translate(status.dwCurrentState)
}

fn translate(state: u32) -> String {
    match state {
        1 => "остановлена",
        2 => "запускается",
        3 => "останавливается",
        4 => "работает",
        5 => "возобновление",
        6 => "приостановка",
        7 => "приостановлена",
        _ => "неизвестно",
    }
    .to_string()
}

pub fn installed(name: &str) -> bool {
    status(name) != "не установлена"
}

/// Полный снимок службы: состояние + путь + сохранённая стратегия.
pub fn query(name: &str) -> ServiceSnapshot {
    let mut snap = ServiceSnapshot {
        name: name.to_string(),
        ..Default::default()
    };
    let status_text = status(name);
    snap.installed = status_text != "не установлена";
    snap.status = status_text;
    if let Some(image) = super::sys::reg_get_string(
        true,
        &format!(r"System\CurrentControlSet\Services\{}", name),
        "ImagePath",
    ) {
        snap.image_path = image;
    }
    if name == "zapret" {
        if let Some(strategy) = super::sys::reg_get_string(
            true,
            r"System\CurrentControlSet\Services\zapret",
            "zapret-discord-youtube",
        ) {
            snap.strategy = strategy;
        }
    }
    snap
}

pub fn stop(name: &str) -> Result<(), String> {
    let scm = open_scm(false)?;
    let svc = open_service(&scm, name, SERVICE_STOP | SERVICE_QUERY_STATUS)
        .map_err(|e| error_text(e))?;
    let mut status: windows_sys::Win32::System::Services::SERVICE_STATUS = unsafe { std::mem::zeroed() };
    unsafe {
        ControlService(svc.0, SERVICE_CONTROL_STOP, &mut status);
    }
    Ok(())
}

pub fn start(name: &str) -> Result<(), String> {
    let scm = open_scm(false)?;
    let svc = open_service(&scm, name, SERVICE_START | SERVICE_QUERY_STATUS)
        .map_err(|e| error_text(e))?;
    let ok = unsafe { StartServiceW(svc.0, 0, std::ptr::null()) };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        // 1056 (уже запущена) не считаем ошибкой
        if err != 1056 {
            return Err(error_text(err));
        }
    }
    Ok(())
}

pub fn delete(name: &str) -> Result<(), String> {
    let scm = open_scm(true)?;
    let svc = open_service(&scm, name, DELETE_ACCESS | SERVICE_QUERY_STATUS)
        .map_err(|e| error_text(e))?;
    let ok = unsafe { DeleteService(svc.0) };
    if ok == 0 {
        return Err(error_text(unsafe { GetLastError() }));
    }
    Ok(())
}

/// Создать и запустить службу с автозапуском.
pub fn create(name: &str, display: &str, bin_path: &str, description: &str) -> Result<(), String> {
    let scm = open_scm(true)?;
    // Если служба уже есть — сначала удаляем.
    if let Ok(old) = open_service(&scm, name, DELETE_ACCESS | SERVICE_STOP | SERVICE_QUERY_STATUS) {
        unsafe {
            let mut status: windows_sys::Win32::System::Services::SERVICE_STATUS = std::mem::zeroed();
            ControlService(old.0, SERVICE_CONTROL_STOP, &mut status);
            DeleteService(old.0);
        }
        // Windows помечает удалённую службу на удаление: ждём, пока освободится имя.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while exists(name) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    }
    let name_w = wide(name);
    let display_w = wide(display);
    let bin_w = wide(bin_path);
    let mut created: SC_HANDLE = std::ptr::null_mut();
    let mut last_err = 0u32;
    for _ in 0..4 {
        created = unsafe {
            CreateServiceW(
                scm.0,
                name_w.as_ptr(),
                display_w.as_ptr(),
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_AUTO_START,
                SERVICE_ERROR_NORMAL,
                bin_w.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if !created.is_null() {
            break;
        }
        last_err = unsafe { GetLastError() };
        // 1072 — служба помечена на удаление, 1073 — уже существует и запускается
        if last_err != 1072 && last_err != 1073 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
    if created.is_null() {
        return Err(format!(
            "Не удалось создать службу (код {}): {}",
            last_err,
            error_text(last_err)
        ));
    }
    let svc = ScHandle(created);
    let desc_w = wide(description);
    let desc = SERVICE_DESCRIPTIONW {
        lpDescription: desc_w.as_ptr() as *mut u16,
    };
    unsafe {
        ChangeServiceConfig2W(
            svc.0,
            SERVICE_CONFIG_DESCRIPTION,
            &desc as *const SERVICE_DESCRIPTIONW as *const core::ffi::c_void,
        );
    }
    let ok = unsafe { StartServiceW(svc.0, 0, std::ptr::null()) };
    if ok == 0 {
        let err = unsafe { GetLastError() };
        if err != 1056 {
            return Err(format!("Служба создана, но не запустилась: {}", error_text(err)));
        }
    }
    Ok(())
}

/// Существует ли служба (по коду ошибки 1060 = не существует).
pub fn exists(name: &str) -> bool {
    match open_scm(false) {
        Ok(scm) => match open_service(&scm, name, SERVICE_QUERY_CONFIG) {
            Ok(_) => true,
            Err(code) => code != ERROR_SERVICE_DOES_NOT_EXIST,
        },
        Err(_) => false,
    }
}

/// Записать понятное имя и описание в ключ службы (включая работающие драйверы).
pub fn label(name: &str, display: &str, description: &str) -> bool {
    let path = format!(r"System\CurrentControlSet\Services\{}", name);
    if let Some(key) = super::sys::RegKey::open(true, &path, true) {
        let a = key.set_string("DisplayName", display);
        let b = key.set_string("Description", description);
        return a || b;
    }
    false
}
