//! Автозапуск через планировщик заданий (задача ZapretManager, ONLOGON, HIGHEST).

use std::path::Path;

use super::process;
use super::sys;

pub const TASK_NAME: &str = "ZapretManager";
pub const AUTOSTART_ARG: &str = "--autostart";

/// Существует ли задача автозапуска.
pub fn is_enabled() -> bool {
    let r = process::run(
        "schtasks.exe",
        &["/Query", "/TN", TASK_NAME, "/FO", "LIST"],
        std::time::Duration::from_secs(15),
    );
    r.exit_code == 0 && !r.all().trim().is_empty()
}

/// Включить автозапуск. Возвращает текст для интерфейса.
pub fn enable(exe: &Path, start_minimized: bool) -> String {
    if !sys::is_admin() {
        return "Для автозапуска нужны права администратора.".to_string();
    }
    if !exe.is_file() {
        return format!("Файл программы не найден: {}", exe.to_string_lossy());
    }
    let args = if start_minimized {
        format!("{} --tray", AUTOSTART_ARG)
    } else {
        AUTOSTART_ARG.to_string()
    };
    let xml = task_xml(exe, &args);
    // Уникальное имя: `%TEMP%` доступен на запись процессам того же
    // пользователя, а `schtasks` запускается уже с повышенными правами.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp = super::paths::temp_dir().join(format!(
        "task-{}-{}.xml",
        std::process::id(),
        stamp
    ));
    if let Err(e) = write_utf16le(&temp, &xml) {
        return format!("Не удалось создать задачу автозапуска: {}", e);
    }
    let r = process::run(
        "schtasks.exe",
        &[
            "/Create",
            "/TN",
            TASK_NAME,
            "/XML",
            &temp.to_string_lossy(),
            "/F",
        ],
        std::time::Duration::from_secs(30),
    );
    let _ = std::fs::remove_file(&temp);
    if r.exit_code != 0 {
        let text = crate::util::shorten(&r.all().replace(['\r', '\n'], " "), 200);
        super::log::warn(format!("schtasks create: {}", r.all()));
        return format!("Не удалось создать задачу автозапуска: {}", text);
    }
    if start_minimized {
        "Автозапуск включён: менеджер будет стартовать свёрнутым в трей.".to_string()
    } else {
        "Автозапуск включён: менеджер будет запускаться при входе в Windows.".to_string()
    }
}

/// Выключить автозапуск.
pub fn disable() -> String {
    let r = process::run(
        "schtasks.exe",
        &["/Delete", "/TN", TASK_NAME, "/F"],
        std::time::Duration::from_secs(20),
    );
    if is_enabled() {
        let text = crate::util::shorten(&r.all().replace(['\r', '\n'], " "), 200);
        return format!("Не удалось удалить задачу автозапуска: {}", text);
    }
    "Автозапуск выключен.".to_string()
}

pub fn open_task_scheduler() {
    let _ = process::spawn_hidden("taskschd.msc", &[], None);
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn task_xml(exe: &Path, args: &str) -> String {
    let command = xml_escape(&exe.to_string_lossy());
    let arguments = xml_escape(args);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.3" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Zapret Manager — ускорение соединения для Discord и YouTube</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <GroupId>S-1-5-32-544</GroupId>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>false</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <DisallowStartOnRemoteAppSession>false</DisallowStartOnRemoteAppSession>
    <UseUnifiedSchedulingEngine>true</UseUnifiedSchedulingEngine>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{}</Command>
      <Arguments>{}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
        command, arguments
    )
}

fn write_utf16le(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;

    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    // `create_new` — чтобы не писать по уже существующему (возможно,
    // подсунутому) файлу или ссылке с тем же именем.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()
}
