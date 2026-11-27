//! Значок в области уведомлений: скрытое окно, Shell_NotifyIcon, собственное меню.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

use windows_sys::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, FillRect, SelectObject, SetBkMode,
    SetTextColor, DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, HFONT, TRANSPARENT,
};
use windows_sys::Win32::UI::Controls::{
    DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODS_DISABLED, ODS_SELECTED, ODT_MENU,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconFromResourceEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyIcon, DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW,
    PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
    SetMenuInfo, TrackPopupMenu, TranslateMessage, HICON, MENUINFO, MF_OWNERDRAW, MIM_BACKGROUND,
    MIM_STYLE, MNS_NOCHECK, MSG, WNDCLASSW, WM_APP, WM_CLOSE, WM_DRAWITEM, WM_LBUTTONDBLCLK,
    WM_MEASUREITEM, WM_NULL, WM_RBUTTONUP,
};

use crate::engine::config::Ini;
use crate::engine::install::ZapretInstall;
use crate::engine::ops::ZapretOps;
use crate::engine::{log, paths, sys, tgproxy};
use crate::engine::sys::wide;

pub const WM_TRAY: u32 = WM_APP + 1;

/// Запрос «Выход» из меню трея. UI-поток проверяет флаг в начале кадра.
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn take_exit_request() -> bool {
    EXIT_REQUESTED.swap(false, Ordering::SeqCst)
}

const TPM_RETURNCMD: u32 = 0x0100;
const TPM_NONOTIFY: u32 = 0x0080;
const TPM_RIGHTBUTTON: u32 = 0x0002;

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
    /// Создался ли значок. При `false` окна трея нет и иконки в панели задач
    /// тоже не будет — сворачивать в трей в этом состоянии нельзя.
    pub fn is_alive(&self) -> bool {
        self.hwnd != 0
    }

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
    // Паника не должна пересекать границу `extern "system"`: это abort всего
    // процесса вместе с несохранёнными настройками.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match msg {
        WM_TRAY => {
            let mouse = lparam as u32;
            if mouse == WM_LBUTTONDBLCLK {
                with_context(|c| c.events.send(TrayEvent::Show));
            } else if mouse == WM_RBUTTONUP {
                show_menu(hwnd);
            }
            0
        }
        WM_MEASUREITEM => measure_menu_item(lparam),
        WM_DRAWITEM => draw_menu_item(lparam),
        WM_CLOSE => {
            remove_icon(hwnd);
            DestroyWindow(hwnd);
            // `DestroyWindow` сам по себе не завершает цикл сообщений:
            // без WM_QUIT поток трея оставался жить с нулём окон.
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }));
    result.unwrap_or_else(|_| {
        log::error("Трей: паника в обработчике окна");
        0
    })
}

const MENU_BG_DARK: u32 = rgb(26, 22, 20);
const MENU_ITEM_HEIGHT: u32 = 34;
const MENU_WIDTH: u32 = 268;

const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
}

#[derive(Clone, Copy)]
struct MenuStyle {
    bg: u32,
    text: u32,
    muted: u32,
    hover: u32,
    accent: u32,
    border: u32,
}

static MENU_STYLE: Mutex<MenuStyle> = Mutex::new(MenuStyle {
    bg: MENU_BG_DARK,
    text: rgb(244, 238, 231),
    muted: rgb(126, 116, 105),
    hover: rgb(50, 43, 36),
    accent: rgb(249, 146, 56),
    border: rgb(52, 44, 37),
});

static MENU_SCALE: Mutex<f32> = Mutex::new(1.0);

fn current_menu_scale() -> f32 {
    MENU_SCALE.lock().map(|s| *s).unwrap_or(1.0)
}

fn menu_scale_for_window(hwnd: HWND) -> f32 {
    let dpi = unsafe {
        let window_dpi = GetDpiForWindow(hwnd);
        let system_dpi = GetDpiForSystem();
        window_dpi.max(system_dpi).max(96)
    };
    (dpi as f32 / 96.0).clamp(1.0, 2.5)
}

struct TrayMenuItem {
    id: u32,
    text: String,
    icon: &'static str,
    check: bool,
    separator: bool,
    enabled: bool,
}

fn set_menu_style(light: bool) {
    let style = if light {
        MenuStyle {
            bg: rgb(255, 255, 255),
            text: rgb(31, 27, 24),
            muted: rgb(126, 116, 105),
            hover: rgb(238, 235, 232),
            accent: rgb(226, 116, 26),
            border: rgb(220, 219, 216),
        }
    } else {
        MenuStyle {
            bg: MENU_BG_DARK,
            text: rgb(244, 238, 231),
            muted: rgb(126, 116, 105),
            hover: rgb(50, 43, 36),
            accent: rgb(249, 146, 56),
            border: rgb(52, 44, 37),
        }
    };
    if let Ok(mut slot) = MENU_STYLE.lock() {
        *slot = style;
    }
}

unsafe fn make_menu_font(face: &str, height: i32) -> HFONT {
    let face = wide(face);
    CreateFontW(
        height,
        0,
        0,
        0,
        400,
        0,
        0,
        0,
        1,
        0,
        0,
        5,
        0,
        face.as_ptr(),
    )
}

unsafe fn draw_menu_item(lparam: LPARAM) -> LRESULT {
    let dis = &*(lparam as *const DRAWITEMSTRUCT);
    if dis.CtlType != ODT_MENU || dis.itemData == 0 {
        return 0;
    }
    let item = &*(dis.itemData as *const TrayMenuItem);
    let scale = current_menu_scale();
    let style = MENU_STYLE
        .lock()
        .map(|s| *s)
        .unwrap_or(MenuStyle {
            bg: MENU_BG_DARK,
            text: rgb(244, 238, 231),
            muted: rgb(126, 116, 105),
            hover: rgb(50, 43, 36),
            accent: rgb(249, 146, 56),
            border: rgb(52, 44, 37),
        });
    let selected = dis.itemState & ODS_SELECTED != 0;
    let disabled = dis.itemState & ODS_DISABLED != 0 || !item.enabled;
    let bg = if selected && !disabled {
        style.hover
    } else {
        style.bg
    };
    let bg_brush = CreateSolidBrush(bg as COLORREF);
    FillRect(dis.hDC, &dis.rcItem, bg_brush);
    let _ = DeleteObject(bg_brush as _);

    if item.separator {
        let mut line = dis.rcItem;
        line.left += (12.0 * scale) as i32;
        line.right -= (12.0 * scale) as i32;
        line.top = (line.top + line.bottom) / 2;
        line.bottom = line.top + (scale.round() as i32).max(1);
        let brush = CreateSolidBrush(style.border as COLORREF);
        FillRect(dis.hDC, &line, brush);
        let _ = DeleteObject(brush as _);
        return 1;
    }

    let text_color = if disabled {
        style.muted
    } else if item.check {
        style.text
    } else {
        style.text
    };
    SetBkMode(dis.hDC, TRANSPARENT as i32);
    let icon_font = make_menu_font("Segoe MDL2 Assets", (16.0 * scale).round() as i32);
    let old = SelectObject(dis.hDC, icon_font as _);
    SetTextColor(dis.hDC, if item.enabled { style.accent } else { style.muted } as COLORREF);
    let mut icon_rect = dis.rcItem;
    icon_rect.left += (14.0 * scale) as i32;
    icon_rect.right = icon_rect.left + (22.0 * scale) as i32;
    let icon = wide(item.icon);
    DrawTextW(
        dis.hDC,
        icon.as_ptr(),
        icon.len().saturating_sub(1) as i32,
        &mut icon_rect,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
    SelectObject(dis.hDC, old);
    let _ = DeleteObject(icon_font as _);

    let text_font = make_menu_font("Segoe UI", (14.0 * scale).round() as i32);
    let old = SelectObject(dis.hDC, text_font as _);
    SetTextColor(dis.hDC, text_color as COLORREF);
    let mut text_rect = dis.rcItem;
    text_rect.left += (42.0 * scale) as i32;
    text_rect.right -= (32.0 * scale) as i32;
    let text = wide(&item.text);
    DrawTextW(
        dis.hDC,
        text.as_ptr(),
        text.len().saturating_sub(1) as i32,
        &mut text_rect,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );
    SelectObject(dis.hDC, old);
    let _ = DeleteObject(text_font as _);

    if item.check {
        let check_font = make_menu_font("Segoe MDL2 Assets", (14.0 * scale).round() as i32);
        let old = SelectObject(dis.hDC, check_font as _);
        SetTextColor(dis.hDC, style.accent as COLORREF);
        let mut check_rect = dis.rcItem;
        check_rect.right -= (14.0 * scale) as i32;
        check_rect.left = check_rect.right - (18.0 * scale) as i32;
        let check = wide("\u{E73E}");
        DrawTextW(
            dis.hDC,
            check.as_ptr(),
            check.len().saturating_sub(1) as i32,
            &mut check_rect,
            DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(dis.hDC, old);
        let _ = DeleteObject(check_font as _);
    }
    1
}

unsafe fn measure_menu_item(lparam: LPARAM) -> LRESULT {
    let mis = &mut *(lparam as *mut MEASUREITEMSTRUCT);
    let separator = mis.itemData != 0
        && (*(mis.itemData as *const TrayMenuItem)).separator;
    let scale = current_menu_scale();
    mis.itemWidth = (MENU_WIDTH as f32 * scale).round() as u32;
    mis.itemHeight = ((if separator { 9.0 } else { MENU_ITEM_HEIGHT as f32 }) * scale)
        .round()
        .max(if separator { 6.0 } else { 28.0 }) as u32;
    1
}

fn show_menu(hwnd: HWND) {
    let Some((state, zapret_on, tg_on)) = with_context(|c| {
        let (zapret_on, tg_on) = c
            .state
            .lock()
            .map(|s| (s.zapret_on, s.tg_on))
            .unwrap_or((false, false));
        (c.state.clone(), zapret_on, tg_on)
    }) else {
        return;
    };
    let theme = Ini::load(&paths::config_path()).get("Theme", "auto");
    let light = !crate::ui::theme::resolve_dark(&theme);
    set_menu_style(light);
    if let Ok(mut scale) = MENU_SCALE.lock() {
        *scale = menu_scale_for_window(hwnd);
    }
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let mut items: Vec<Box<TrayMenuItem>> = vec![
            Box::new(TrayMenuItem {
                id: 1,
                text: "Открыть Zapret Manager".to_string(),
                icon: "\u{E8A7}",
                check: false,
                separator: false,
                enabled: true,
            }),
            Box::new(TrayMenuItem {
                id: 0,
                text: String::new(),
                icon: "",
                check: false,
                separator: true,
                enabled: false,
            }),
            Box::new(TrayMenuItem {
                id: 2,
                text: if zapret_on {
                    "Остановить ускорение".to_string()
                } else {
                    "Запустить ускорение".to_string()
                },
                icon: "\u{E7E8}",
                check: zapret_on,
                separator: false,
                enabled: true,
            }),
            Box::new(TrayMenuItem {
                id: 3,
                text: if tg_on {
                    "Остановить TG WS Proxy".to_string()
                } else {
                    "Запустить TG WS Proxy".to_string()
                },
                icon: "\u{E8BD}",
                check: tg_on,
                separator: false,
                enabled: true,
            }),
            Box::new(TrayMenuItem {
                id: 0,
                text: String::new(),
                icon: "",
                check: false,
                separator: true,
                enabled: false,
            }),
            Box::new(TrayMenuItem {
                id: 4,
                text: "Проверить обновления".to_string(),
                icon: "\u{E896}",
                check: false,
                separator: false,
                enabled: true,
            }),
            Box::new(TrayMenuItem {
                id: 0,
                text: String::new(),
                icon: "",
                check: false,
                separator: true,
                enabled: false,
            }),
            Box::new(TrayMenuItem {
                id: 5,
                text: "Выход".to_string(),
                icon: "\u{E8BB}",
                check: false,
                separator: false,
                enabled: true,
            }),
        ];
        for item in &mut items {
            let ptr = (&mut **item) as *mut TrayMenuItem as *const u16;
            AppendMenuW(menu, MF_OWNERDRAW, item.id as usize, ptr);
        }
        let style = MENU_STYLE.lock().map(|s| *s).unwrap_or(MenuStyle {
            bg: MENU_BG_DARK,
            text: rgb(244, 238, 231),
            muted: rgb(126, 116, 105),
            hover: rgb(50, 43, 36),
            accent: rgb(249, 146, 56),
            border: rgb(52, 44, 37),
        });
        let brush = CreateSolidBrush(style.bg as COLORREF);
        let info = MENUINFO {
            cbSize: std::mem::size_of::<MENUINFO>() as u32,
            fMask: MIM_BACKGROUND | MIM_STYLE,
            dwStyle: MNS_NOCHECK,
            cyMax: 0,
            hbrBack: brush,
            dwContextHelpID: 0,
            dwMenuData: 0,
        };
        let _ = SetMenuInfo(menu, &info);

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
        let _ = DeleteObject(brush as _);
        match cmd {
            1 => {
                // Показываем окно напрямую: если оно скрыто в трей, цикл egui
                // может не получать кадры, и событие из канала не обработается.
                sys::activate_window("Zapret Manager");
                send_event(TrayEvent::Show);
            }
            2 => toggle_zapret(state),
            3 => toggle_tg(state),
            4 => {
                sys::activate_window("Zapret Manager");
                send_event(TrayEvent::CheckUpdates);
            }
            5 => {
                EXIT_REQUESTED.store(true, Ordering::SeqCst);
                sys::activate_window("Zapret Manager");
                send_event(TrayEvent::Exit);
            }
            _ => {}
        }
        drop(items);
    }
}

fn send_event(event: TrayEvent) {
    with_context(|c| {
        let _ = c.events.send(event);
    });
}

/// Переключить zapret из трея, не дожидаясь кадра UI.
fn toggle_zapret(state: Arc<Mutex<TrayState>>) {
    std::thread::spawn(move || {
        let ini = Ini::load(&paths::config_path());
        let install = ZapretInstall::detect(&ini.get("Root", ""));
        if !install.is_valid() {
            log::warn("Меню трея: папка zapret не найдена.");
            return;
        }
        let ops = ZapretOps::new(install);
        let before = ops.get_state();
        let message = if before.winws_running {
            ops.stop()
        } else if let Some(strategy) = ops.pick_strategy(&ini.get("LastStrategy", "")) {
            ops.start(strategy).message
        } else {
            "Меню трея: стратегии не найдены.".to_string()
        };
        log::info(format!("Меню трея: {}", message));
        let after = ops.get_state();
        if let Ok(mut st) = state.lock() {
            st.zapret_on = after.winws_running;
        }
    });
}

/// Переключить TG WS Proxy из трея.
fn toggle_tg(state: Arc<Mutex<TrayState>>) {
    std::thread::spawn(move || {
        let ini = Ini::load(&paths::config_path());
        let before = tgproxy::get_state(&ini);
        let message = if before.running {
            tgproxy::stop(&before.dir)
        } else {
            tgproxy::start(&ini).0
        };
        log::info(format!("Меню трея TG WS Proxy: {}", message));
        let after = tgproxy::get_state(&ini);
        if let Ok(mut st) = state.lock() {
            st.tg_on = after.running;
        }
    });
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
    // Иконка создана через CreateIconFromResourceEx — её нужно уничтожить,
    // иначе дескриптор утекает на каждый перезапуск потока трея.
    if handle != 0 {
        DestroyIcon(handle as HICON);
    }
    PostQuitMessage(0);
}

