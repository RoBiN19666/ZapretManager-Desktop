//! Встраивание иконки и сведений о версии в exe (Windows).
//!
//! Для GNU-тулчейна используется `windres`, для MSVC — `rc.exe`.
//! Без этого Explorer показывает у exe стандартную пустую иконку, хотя
//! оконная иконка и значок в трее задаются во время выполнения.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/zapret-manager.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest.join("assets").join("zapret-manager.ico");
    if !icon.is_file() {
        println!(
            "cargo:warning=assets/zapret-manager.ico не найден — exe останется без иконки"
        );
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
    let (major, minor, patch) = version_parts(&version);
    let icon_rc = icon.to_string_lossy().replace('\\', "/");
    let rc_path = out.join("zapret-manager.rc");
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{icon_rc}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x40004L
FILETYPE 0x1L
{{
  BLOCK "StringFileInfo"
  {{
    BLOCK "040904B0"
    {{
      VALUE "CompanyName", "RoBiN19666"
      VALUE "FileDescription", "Zapret Manager"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "ZapretManager"
      VALUE "LegalCopyright", "MIT"
      VALUE "OriginalFilename", "ZapretManager.exe"
      VALUE "ProductName", "Zapret Manager"
      VALUE "ProductVersion", "{version}"
    }}
  }}
  BLOCK "VarFileInfo"
  {{
    VALUE "Translation", 0x409, 1200
  }}
}}
"#
    );
    if let Err(error) = std::fs::write(&rc_path, rc) {
        println!("cargo:warning=не удалось записать .rc: {error}");
        return;
    }

    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env == "gnu" {
        let Some(windres) = find_windres() else {
            println!("cargo:warning=windres не найден — exe останется без иконки");
            return;
        };
        let obj = out.join("zapret-manager-res.o");
        let status = Command::new(&windres)
            .arg("-I")
            .arg(manifest.join("assets"))
            .arg("-i")
            .arg(&rc_path)
            .arg("-o")
            .arg(&obj)
            .arg("-O")
            .arg("coff")
            .status();
        match status {
            Ok(s) if s.success() && obj.is_file() => {
                println!("cargo:rustc-link-arg-bins={}", obj.display());
            }
            Ok(s) => println!("cargo:warning=windres завершился с кодом {s} — иконка не встроена"),
            Err(error) => println!("cargo:warning=не удалось запустить windres: {error}"),
        }
    } else if target_env == "msvc" {
        let Some(rc_exe) = find_rc() else {
            println!("cargo:warning=rc.exe не найден — exe останется без иконки");
            return;
        };
        let res = out.join("zapret-manager-res.res");
        let status = Command::new(&rc_exe)
            .arg("/nologo")
            .arg("/fo")
            .arg(&res)
            .arg(&rc_path)
            .status();
        match status {
            Ok(s) if s.success() && res.is_file() => {
                println!("cargo:rustc-link-arg-bins={}", res.display());
            }
            Ok(s) => println!("cargo:warning=rc.exe завершился с кодом {s} — иконка не встроена"),
            Err(error) => println!("cargo:warning=не удалось запустить rc.exe: {error}"),
        }
    }
}

fn version_parts(version: &str) -> (u32, u32, u32) {
    let mut parts = version.split('.').map(|p| {
        p.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .unwrap_or(0)
    });
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

fn find_windres() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("WINDRES") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let prefix = match arch.as_str() {
        "x86_64" => "x86_64-w64-mingw32",
        "x86" => "i686-w64-mingw32",
        "aarch64" => "aarch64-w64-mingw32",
        _ => "",
    };
    let mut candidates: Vec<String> = Vec::new();
    if !prefix.is_empty() {
        candidates.push(format!("{prefix}-windres"));
        candidates.push(format!("{prefix}-windres.exe"));
    }
    candidates.push("windres".to_string());
    candidates.push("windres.exe".to_string());
    candidates
        .into_iter()
        .find(|name| Command::new(name).arg("--version").output().is_ok())
        .map(PathBuf::from)
}

fn find_rc() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("RC") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if Command::new("rc").arg("/?").output().is_ok() {
        return Some(PathBuf::from("rc"));
    }
    // Windows SDK: ...\Windows Kits\10\bin\<version>\<arch>\rc.exe
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default().as_str() {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        _ => return None,
    };
    let roots = [
        std::env::var("ProgramFiles(x86)").ok(),
        std::env::var("ProgramFiles").ok(),
    ];
    for root in roots.into_iter().flatten() {
        let bin = Path::new(&root).join("Windows Kits").join("10").join("bin");
        let Ok(entries) = std::fs::read_dir(&bin) else {
            continue;
        };
        let mut versions: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        versions.sort();
        for version in versions.into_iter().rev() {
            let candidate = version.join(arch).join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
