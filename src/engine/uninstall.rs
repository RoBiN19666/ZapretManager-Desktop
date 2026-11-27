//! Удаление компонентов и очистка данных Zapret Manager.

use std::path::{Path, PathBuf};

use crate::engine::config::Ini;
use crate::engine::install::ZapretInstall;
use crate::engine::ops::ZapretOps;
use crate::engine::{autostart, paths, process, sys, tgproxy, tgsource};

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn remove_dir(path: &Path, label: &str, log: &mut String) {
    if !path.exists() {
        return;
    }
    match std::fs::remove_dir_all(path) {
        Ok(()) => log.push_str(&format!("{} удалена. ", label)),
        Err(e) => log.push_str(&format!("Не удалось удалить {}: {}. ", label, e)),
    }
}

fn remove_file(path: &Path, label: &str, log: &mut String) {
    if !path.exists() {
        return;
    }
    match std::fs::remove_file(path) {
        Ok(()) => log.push_str(&format!("{} удалён. ", label)),
        Err(e) => log.push_str(&format!("Не удалось удалить {}: {}. ", label, e)),
    }
}

/// Лежит ли `child` внутри `ancestor` (сравнение без учёта регистра).
fn is_inside(child: &Path, ancestor: &Path) -> bool {
    let child = child.to_string_lossy().to_lowercase();
    let ancestor = ancestor.to_string_lossy().to_lowercase();
    if ancestor.is_empty() {
        return false;
    }
    child == ancestor
        || (child.starts_with(&ancestor)
            && child[ancestor.len()..].starts_with('\\'))
}

/// Можно ли удалять каталог, взятый из `TgDir`.
///
/// `TgDir` живёт в `manager.ini`, который может быть подменён (в том числе
/// восстановлением чужого `.zmb`), поэтому `remove_dir_all` по нему без
/// проверок — это удаление произвольного каталога. Требуем: абсолютный путь,
/// не корень диска, не каталог программы и не его родитель, и чтобы внутри
/// действительно лежал `tgwsproxy*.exe`.
fn safe_tg_dir(dir: &Path) -> bool {
    if !dir.is_absolute() || dir.to_string_lossy().len() <= 3 {
        return false;
    }
    let exe_dir = paths::exe_dir();
    if is_inside(dir, &exe_dir) || is_inside(&exe_dir, dir) {
        return false;
    }
    tgproxy::find_exe_in(dir).is_some()
}

/// Удалить сборку zapret и связанные службы/записи hosts.
/// При `keep_lists` папка lists сохраняется в data\removed-zapret-lists.
pub fn remove_zapret(install: &ZapretInstall, keep_lists: bool) -> Result<String, String> {
    if !sys::is_admin() {
        return Err("Удаление zapret требует прав администратора.".to_string());
    }
    let root = Path::new(&install.root);
    if install.root.trim().is_empty() || !install.is_valid() || !root.is_dir() {
        return Err("Папка zapret не найдена — удалять нечего.".to_string());
    }
    if root.parent().is_none() || root.to_string_lossy().len() <= 3 {
        return Err("Отказано: путь похож на корень диска.".to_string());
    }
    let mut message = String::new();
    let ops = ZapretOps::new(install.clone());
    message.push_str(&ops.remove_service());
    message.push(' ');
    let hosts = ops.clear_hosts_block();
    if !hosts.is_empty() {
        message.push_str(&hosts);
        message.push(' ');
    }
    if keep_lists && install.lists_dir().is_dir() {
        let dst = paths::data_dir().join("removed-zapret-lists");
        match copy_dir(&install.lists_dir(), &dst) {
            Ok(()) => message.push_str(&format!(
                "Списки сохранены в {}. ",
                dst.to_string_lossy()
            )),
            Err(e) => {
                return Err(format!(
                    "Не удалось сохранить списки, удаление отменено: {}",
                    e
                ))
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(400));
    match std::fs::remove_dir_all(root) {
        Ok(()) => message.push_str("Папка zapret удалена. "),
        Err(e) => {
            return Err(format!(
                "Служба и hosts очищены, но папку удалить не удалось: {}. Закройте winws.exe и повторите.",
                e
            ))
        }
    }
    let mut ini = Ini::load(&paths::config_path());
    ini.set("Root", "");
    let _ = ini.save();
    Ok(message.trim().to_string())
}

/// Удалить TG WS Proxy, тихий режим и его данные.
pub fn remove_tg(ini: &mut Ini) -> Result<String, String> {
    let mut message = String::new();
    let dir = tgproxy::dir_or_default(ini);
    message.push_str(&tgproxy::stop_all());
    message.push(' ');
    let _ = tgsource::stop();
    if safe_tg_dir(&dir) {
        remove_dir(&dir, "папка TG WS Proxy", &mut message);
    } else {
        message.push_str(
            "Папка TG WS Proxy из настроек не похожа на установку — удаление пропущено. ",
        );
    }
    remove_dir(&tgsource::root_dir(), "исходники TG WS Proxy", &mut message);

    let appdata = std::env::var("APPDATA").unwrap_or_default();
    if !appdata.is_empty() {
        remove_dir(
            &Path::new(&appdata).join("TgWsProxy"),
            "данные TgWsProxy",
            &mut message,
        );
    }
    if let Ok(entries) = std::fs::read_dir(tgproxy::startup_folder()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase().starts_with("tgwsproxy"))
                .unwrap_or(false)
            {
                remove_file(&path, "автозапуск TG WS Proxy", &mut message);
            }
        }
    }
    for key in ["TgExe", "TgDir", "TgVersion", "TgRunMode"] {
        ini.set(key, "");
    }
    let _ = ini.save();
    if message.trim().is_empty() {
        message = "TG WS Proxy не был установлен.".to_string();
    }
    Ok(message.trim().to_string())
}

/// Путь для `.cmd`: `%` раскрывается cmd даже внутри кавычек, поэтому удваиваем.
fn cmd_path(path: &Path) -> String {
    path.to_string_lossy().replace('%', "%%")
}

fn write_cleanup_script(remove_exe: bool, remove_data: bool) -> Result<PathBuf, String> {
    use std::io::Write as _;

    let exe = paths::exe_path();
    let data = paths::data_dir();
    // Уникальное имя + create_new: `%TEMP%` доступен на запись другим процессам
    // пользователя, а cmd.exe запускается уже с повышенными правами.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let script = paths::temp_dir().join(format!(
        "zm-cleanup-{}-{}.cmd",
        std::process::id(),
        stamp
    ));
    let mut text = String::from("@echo off\r\n");
    text.push_str("timeout /t 3 /nobreak >nul\r\n");
    if remove_data {
        text.push_str(&format!("rmdir /s /q \"{}\"\r\n", cmd_path(&data)));
    }
    if remove_exe {
        text.push_str(&format!("del /f /q \"{}\"\r\n", cmd_path(&exe)));
    }
    text.push_str("del /f /q \"%~f0\"\r\n");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&script)
        .map_err(|e| e.to_string())?;
    file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    drop(file);
    process::spawn_hidden(
        "cmd.exe",
        &["/c", &script.to_string_lossy()],
        Some(&paths::temp_dir()),
    )
    .map_err(|e| e.to_string())?;
    Ok(script)
}

/// Удалить всё: компоненты, службы, автозапуск, данные и сам exe после выхода.
pub fn remove_everything(install: &ZapretInstall, ini: &mut Ini) -> Result<String, String> {
    if !sys::is_admin() {
        return Err("Полное удаление требует прав администратора.".to_string());
    }
    let mut message = String::new();
    if install.is_valid() {
        match remove_zapret(install, false) {
            Ok(m) => message.push_str(&m),
            Err(e) => message.push_str(&format!("zapret: {}. ", e)),
        }
    } else {
        let ops = ZapretOps::new(install.clone());
        message.push_str(&ops.remove_service());
        message.push(' ');
        let _ = ops.clear_hosts_block();
    }
    match remove_tg(ini) {
        Ok(m) => message.push_str(&m),
        Err(e) => message.push_str(&format!("TG WS Proxy: {}. ", e)),
    }
    message.push_str(&autostart::disable());
    message.push(' ');
    write_cleanup_script(true, true)?;
    message.push_str("Данные и exe будут удалены после закрытия приложения.");
    Ok(message.trim().to_string())
}

/// Очистить только данные менеджера: настройки, журнал, копии, отчёты.
/// Приложение закроется, чтобы файлы не были заняты.
pub fn clean_manager_data() -> Result<String, String> {
    let message = autostart::disable();
    write_cleanup_script(false, true)?;
    Ok(format!(
        "{}. Настройки, журнал, резервные копии и отчёты будут удалены после закрытия.",
        message.trim_end_matches('.')
    ))
}
