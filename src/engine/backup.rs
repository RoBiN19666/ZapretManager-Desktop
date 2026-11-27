//! Резервная копия настроек менеджера и пользовательских файлов zapret.
//!
//! Формат `.zmb` — обычный ZIP с `manifest.json`:
//!   manager/manager.ini
//!   zapret/lists/list-*-user.txt
//!   zapret/lists/ipset-all.txt (+ .backup)
//!   zapret/utils/game_filter.enabled
//!   zapret/utils/check_updates.enabled
//!   zapret/utils/targets.txt

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::config::Ini;
use super::install::ZapretInstall;
use super::paths;
use super::sys;

const FORMAT: &str = "zapret-manager-backup";
const VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";

pub struct BackupInfo {
    pub path: PathBuf,
    pub files: usize,
}

pub struct RestoreInfo {
    pub files: usize,
}

fn backups_dir() -> PathBuf {
    paths::data_dir().join("backups")
}

fn stamp() -> String {
    let now = sys::local_now();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        now.year, now.month, now.day, now.hour, now.minute, now.second
    )
}

/// Имя файла для диалога «Сохранить».
pub fn suggested_name() -> String {
    format!("zapret-manager-{}.zmb", stamp())
}

fn add_file(
    zip: &mut ZipWriter<File>,
    entry: &str,
    path: &Path,
    files: &mut Vec<String>,
) -> Result<(), String> {
    if !path.is_file() {
        return Ok(());
    }
    let data = std::fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(entry, options)
        .map_err(|e| e.to_string())?;
    zip.write_all(&data).map_err(|e| e.to_string())?;
    files.push(entry.to_string());
    Ok(())
}

/// Создать копию в указанный файл.
pub fn create(install: &ZapretInstall, target: &Path) -> Result<BackupInfo, String> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = File::create(target).map_err(|e| format!("Не удалось создать копию: {}", e))?;
    let mut zip = ZipWriter::new(file);
    let mut files = Vec::new();

    let manifest = serde_json::json!({
        "format": FORMAT,
        "version": VERSION,
        "app": crate::APP_VERSION,
        "created": format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", sys::local_now().year, sys::local_now().month, sys::local_now().day, sys::local_now().hour, sys::local_now().minute, sys::local_now().second),
    });
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(MANIFEST, options)
        .map_err(|e| e.to_string())?;
    zip.write_all(manifest.to_string().as_bytes())
        .map_err(|e| e.to_string())?;

    add_file(&mut zip, "manager/manager.ini", &paths::config_path(), &mut files)?;
    let lists = install.lists_dir();
    for name in [
        "list-general-user.txt",
        "list-exclude-user.txt",
        "ipset-exclude-user.txt",
        "ipset-all.txt",
        "ipset-all.txt.backup",
    ] {
        add_file(
            &mut zip,
            &format!("zapret/lists/{}", name),
            &lists.join(name),
            &mut files,
        )?;
    }
    let utils = install.utils_dir();
    for name in [
        "game_filter.enabled",
        "check_updates.enabled",
        "targets.txt",
    ] {
        add_file(
            &mut zip,
            &format!("zapret/utils/{}", name),
            &utils.join(name),
            &mut files,
        )?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(BackupInfo {
        path: target.to_path_buf(),
        files: files.len(),
    })
}

/// Создать автоматическую копию в `data/backups` и оставить 5 последних.
pub fn auto_backup(install: &ZapretInstall, prefix: &str) -> Result<PathBuf, String> {
    let dir = backups_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}-{}.zmb", prefix, stamp()));
    create(install, &path)?;
    let _ = prune(&dir, 5);
    Ok(path)
}

fn prune(dir: &Path, keep: usize) -> std::io::Result<()> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            if !p.extension().map(|e| e.eq_ignore_ascii_case("zmb")).unwrap_or(false) {
                return false;
            }
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            // Ручные копии пользователя не удаляем.
            name.starts_with("pre-restore-") || name.starts_with("auto-")
        })
        .collect();
    files.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    while files.len() > keep {
        let old = files.remove(0);
        let _ = std::fs::remove_file(old);
    }
    Ok(())
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("restore-tmp");
    std::fs::write(&temp, data).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        e.to_string()
    })
}

fn target_for(install: &ZapretInstall, entry: &str, config: &Path) -> Option<PathBuf> {
    match entry {
        "manager/manager.ini" => Some(config.to_path_buf()),
        "zapret/lists/list-general-user.txt" => Some(install.lists_dir().join("list-general-user.txt")),
        "zapret/lists/list-exclude-user.txt" => Some(install.lists_dir().join("list-exclude-user.txt")),
        "zapret/lists/ipset-exclude-user.txt" => Some(install.lists_dir().join("ipset-exclude-user.txt")),
        "zapret/lists/ipset-all.txt" => Some(install.lists_dir().join("ipset-all.txt")),
        "zapret/lists/ipset-all.txt.backup" => Some(install.lists_dir().join("ipset-all.txt.backup")),
        "zapret/utils/game_filter.enabled" => Some(install.utils_dir().join("game_filter.enabled")),
        "zapret/utils/check_updates.enabled" => {
            Some(install.utils_dir().join("check_updates.enabled"))
        }
        "zapret/utils/targets.txt" => Some(install.utils_dir().join("targets.txt")),
        _ => None,
    }
}

/// Восстановить копию. Перед изменением файлов создаётся `pre-restore` копия.
pub fn restore(install: &ZapretInstall, source: &Path) -> Result<RestoreInfo, String> {
    restore_impl(install, source, &paths::config_path(), true)
}

fn restore_impl(
    install: &ZapretInstall,
    source: &Path,
    config: &Path,
    make_backup: bool,
) -> Result<RestoreInfo, String> {
    let file = File::open(source).map_err(|e| format!("Не удалось открыть копию: {}", e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| format!("Это не файл .zmb: {}", e))?;
    let mut manifest_ok = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.name() == MANIFEST {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|e| e.to_string())?;
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|e| format!("manifest.json: {}", e))?;
            manifest_ok = value.get("format").and_then(|v| v.as_str()) == Some(FORMAT);
            break;
        }
    }
    if !manifest_ok {
        return Err("В файле нет манифеста Zapret Manager.".to_string());
    }

    // Текущее состояние — в автоматическую копию, чтобы можно было отменить.
    if make_backup {
        let _ = auto_backup(install, "pre-restore");
    }

    let mut restored = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(target) = target_for(install, entry.name(), config) else {
            continue;
        };
        if entry.size() > 8 * 1024 * 1024 {
            return Err(format!("Файл {} слишком большой.", entry.name()));
        }
        let mut data = Vec::new();
        entry.read_to_end(&mut data).map_err(|e| e.to_string())?;
        write_atomic(&target, &data)?;
        restored += 1;
    }
    if restored == 0 {
        return Err("В копии нет известных файлов.".to_string());
    }
    Ok(RestoreInfo { files: restored })
}

/// Загрузить `Root` из восстановленного manager.ini.
pub fn restored_root() -> String {
    Ini::load(&paths::config_path()).get("Root", "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_valid_backup() {
        let root = std::env::temp_dir().join(format!("zapret-backup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("lists")).unwrap();
        std::fs::create_dir_all(root.join("utils")).unwrap();
        std::fs::write(root.join("lists").join("list-general-user.txt"), "example.com\n").unwrap();
        std::fs::write(root.join("utils").join("targets.txt"), "[YouTube]\nyoutube.com\n").unwrap();
        std::fs::write(root.join("lists").join("ipset-all.txt"), "1.2.3.4/32\n").unwrap();
        std::fs::write(root.join("utils").join("check_updates.enabled"), "ENABLED").unwrap();
        let install = ZapretInstall {
            root: root.to_string_lossy().to_string(),
            ..Default::default()
        };
        let target = root.join("backup.zmb");
        let info = create(&install, &target).unwrap();
        assert!(info.files >= 4);
        let file = File::open(&target).unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        let mut manifest = String::new();
        zip.by_name(MANIFEST)
            .unwrap()
            .read_to_string(&mut manifest)
            .unwrap();
        assert!(manifest.contains(FORMAT));
        assert!(zip.by_name("zapret/lists/list-general-user.txt").is_ok());
        assert!(zip.by_name("zapret/utils/targets.txt").is_ok());
        assert!(zip.by_name("zapret/lists/ipset-all.txt").is_ok());
        assert!(zip.by_name("zapret/utils/check_updates.enabled").is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn restores_files_without_touching_other_dirs() {
        let root = std::env::temp_dir().join(format!("zapret-restore-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("lists")).unwrap();
        std::fs::create_dir_all(root.join("utils")).unwrap();
        let config = root.join("manager.ini");
        std::fs::write(&config, "Root=old\n").unwrap();
        let zip_path = root.join("restore.zmb");
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opt = SimpleFileOptions::default();
        zip.start_file(MANIFEST, opt).unwrap();
        zip.write_all(br#"{"format":"zapret-manager-backup","version":1}"#).unwrap();
        zip.start_file("manager/manager.ini", opt).unwrap();
        zip.write_all(b"Root=new\nLastStrategy=general\n").unwrap();
        zip.start_file("zapret/lists/list-general-user.txt", opt).unwrap();
        zip.write_all(b"example.org\n").unwrap();
        zip.finish().unwrap();

        let install = ZapretInstall {
            root: root.to_string_lossy().to_string(),
            ..Default::default()
        };
        let info = restore_impl(&install, &zip_path, &config, false).unwrap();
        assert_eq!(info.files, 2);
        assert!(std::fs::read_to_string(&config).unwrap().contains("LastStrategy=general"));
        assert!(std::fs::read_to_string(root.join("lists").join("list-general-user.txt"))
            .unwrap()
            .contains("example.org"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
