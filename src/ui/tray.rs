//! Значок в области уведомлений: скрытое окно, Shell_NotifyIcon, собственное меню.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconFromResourceEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW,
    PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
    TrackPopupMenu, TranslateMessage, MSG, WNDCLASSW, WM_APP, WM_CLOSE, WM_LBUTTONDBLCLK,
    WM_NULL, WM_RBUTTONUP,
};

use super::super::engine::sys::wide;

pub const WM_TRAY: u32 = WM_APP + 1;

const TPM_RETURNCMD: u32 = 0x0100;
const TPM_NONOTIFY: u32 = 0x0080;
const TPM_RIGHTBUTTON: u32 = 0x0002;
const MF_STRING: u32 = 0x0000;
const MF_SEPARATOR: u32 = 0x0800;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayEvent {
    Show,
    ToggleZapret,
    ToggleTg,
    CheckUpdates,
    Exit,
}

#[derive(Debug, Clone, Default)]
pub struct TrayState {
    pub zapret_on: bool,
    pub tg_on: bool,
}

pub struct TrayHandle {
    hwnd: isize,
}

impl TrayHandle {
    pub fn close(&self) {
        if self.hwnd != 0 {
            unsafe {
                PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0);
            }
        }
    }
}

/// Создать значок в трее в отдельном потоке.
pub fn spawn(icon: Option<Vec<u8>>, state: Arc<Mutex<TrayState>>) -> (Receiver<TrayEvent>, TrayHandle) {
    let (tx, rx) = mpsc::channel();
    let (hwnd_tx, hwnd_rx) = mpsc::channel::<isize>();
    std::thread::spawn(move || unsafe {
        run_tray(icon, state, tx, hwnd_tx);
    });
    let hwnd = hwnd_rx.recv_timeout(std::time::Duration::from_secs(3)).unwrap_or(0);
    (rx, TrayHandle { hwnd })
}

struct TrayContext {
    icon: Option<Vec<u8>>,
    state: Arc<Mutex<TrayState>>,
    events: Sender<TrayEvent>,
}

static CONTEXT: Mutex<Option<TrayContext>> = Mutex::new(None);

fn with_context<T>(f: impl FnOnce(&TrayContext) -> T) -> Option<T> {
    CONTEXT.lock().ok().and_then(|c| c.as_ref().map(f))
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            let mouse = lparam as u32;
            if mouse == WM_LBUTTONDBLCLK {
                with_context(|c| c.events.send(TrayEvent::Show));
            } else if mouse == WM_RBUTTONUP {
                show_menu(hwnd);
            }
            0
        }
        WM_CLOSE => {
            remove_icon(hwnd);
            DestroyWindow(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn show_menu(hwnd: HWND) {
    let Some((zapret_on, tg_on)) = with_context(|c| {
        c.state
            .lock()
            .map(|s| (s.zapret_on, s.tg_on))
            .unwrap_or((false, false))
    }) else {
        return;
    };
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let mut add = |flags: u32, id: usize, text: &str| {
            let w = wide(text);
            AppendMenuW(menu, flags, id, w.as_ptr());
        };
        add(MF_STRING, 1, "Открыть Zapret Manager");
        add(MF_SEPARATOR, 0, "");
        add(
            MF_STRING,
            2,
            if zapret_on {
                "Остановить zapret"
            } else {
                "Запустить zapret"
            },
        );
        add(
            MF_STRING,
            3,
            if tg_on {
                "Остановить TG WS Proxy"
            } else {
                "Запустить TG WS Proxy"
            },
        );
        add(MF_SEPARATOR, 0, "");
        add(MF_STRING, 4, "Проверить обновления");
        add(MF_SEPARATOR, 0, "");
        add(MF_STRING, 5, "Выход");

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        let event = match cmd {
            1 => Some(TrayEvent::Show),
            2 => Some(TrayEvent::ToggleZapret),
            3 => Some(TrayEvent::ToggleTg),
            4 => Some(TrayEvent::CheckUpdates),
            5 => Some(TrayEvent::Exit),
            _ => None,
        };
        if let Some(event) = event {
            with_context(|c| c.events.send(event));
        }
    }
}

fn icon_handle(bytes: Option<&[u8]>) -> isize {
    if let Some(bytes) = bytes {
        if bytes.len() > 22 {
            let _reserved = u16::from_le_bytes([bytes[0], bytes[1]]);
            let kind = u16::from_le_bytes([bytes[2], bytes[3]]);
            let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
            if kind == 1 && count > 0 {
                let mut best: Option<(u32, usize, usize)> = None;
                for i in 0..count {
                    let off = 6 + i * 16;
                    if off + 16 > bytes.len() {
                        break;
                    }
                    let w = bytes[off] as u32;
                    let h = bytes[off + 1] as u32;
                    let size = u32::from_le_bytes([
                        bytes[off + 8],
                        bytes[off + 9],
                        bytes[off + 10],
                        bytes[off + 11],
                    ]) as usize;
                    let data_off = u32::from_le_bytes([
                        bytes[off + 12],
                        bytes[off + 13],
                        bytes[off + 14],
                        bytes[off + 15],
                    ]) as usize;
                    let area = if w == 0 { 256 } else { w } * if h == 0 { 256 } else { h };
                    if best.map(|(a, _, _)| area >= a).unwrap_or(true) && data_off + size <= bytes.len() {
                        best = Some((area, data_off, size));
                    }
                }
                if let Some((_, off, size)) = best {
                    unsafe {
                        let hicon = CreateIconFromResourceEx(
                            bytes[off..].as_ptr(),
                            size as u32,
                            1,
                            0x0003_0000,
                            0,
                            0,
                            0,
                        );
                        if !hicon.is_null() {
                            return hicon as isize;
                        }
                    }
                }
            }
        }
    }
    unsafe {
        let fallback = LoadIconW(std::ptr::null_mut(), 32512 as *const u16);
        fallback as isize
    }
}

unsafe fn add_icon(hwnd: HWND, icon: isize) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = icon as *mut core::ffi::c_void;
    let tip: Vec<u16> = "Zapret Manager".encode_utf16().collect();
    for (i, c) in tip.iter().take(127).enumerate() {
        nid.szTip[i] = *c;
    }
    Shell_NotifyIconW(NIM_ADD, &nid);
}

unsafe fn remove_icon(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe fn run_tray(
    icon: Option<Vec<u8>>,
    state: Arc<Mutex<TrayState>>,
    events: Sender<TrayEvent>,
    hwnd_tx: Sender<isize>,
) {
    if let Ok(mut ctx) = CONTEXT.lock() {
        *ctx = Some(TrayContext {
            icon,
            state,
            events,
        });
    }
    let instance = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
    let class_name = wide("ZapretManagerTrayWindow");
    let wc = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
    };
    RegisterClassW(&wc);
    let hwnd = CreateWindowExW(
        0,
        class_name.as_ptr(),
        class_name.as_ptr(),
        0,
        0,
        0,
        0,
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        instance,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        let _ = hwnd_tx.send(0);
        return;
    }
    let icon = with_context(|c| c.icon.clone()).flatten();
    let handle = icon_handle(icon.as_deref());
    add_icon(hwnd, handle);
    let _ = hwnd_tx.send(hwnd as isize);

    let taskbar_created = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());

    let mut msg: MSG = std::mem::zeroed();
    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
        if msg.message == taskbar_created {
            add_icon(hwnd, handle);
            continue;
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    remove_icon(hwnd);
    PostQuitMessage(0);
}

/// Обновить состояние для меню (подписи «Запустить/Остановить»).
pub fn set_state(state: &Arc<Mutex<TrayState>>) {
    let _ = state;
}

/// Перерисовать значок (после смены темы и т.п.).
pub fn refresh_icon() {
    with_context(|c| {
        if let Ok(ctx) = CONTEXT.lock() {
            let _ = c;
            drop(ctx);
        }
    });
}

