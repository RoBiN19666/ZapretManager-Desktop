//! Разбор .bat-файла стратегии в аргументы командной строки winws.exe.

use std::path::Path;

use super::gamefilter::GameFilterState;
use super::install::{StrategyDef, ZapretInstall};
use super::textfile;

/// Аргументы запуска winws.exe, собранные из .bat-файла.
pub fn get_argv(
    inst: &ZapretInstall,
    strategy: &StrategyDef,
    gf: &GameFilterState,
) -> Result<Vec<String>, String> {
    let lines = textfile::read_lines(&strategy.file);
    if lines.is_empty() && !Path::new(&strategy.file).exists() {
        return Err("Не удалось прочитать файл стратегии: файл не найден".to_string());
    }

    let mut vars: Vec<(String, String)> = vec![
        ("GameFilterTCP".to_string(), gf.tcp_token()),
        ("GameFilterUDP".to_string(), gf.udp_token()),
        ("GameFilter".to_string(), gf.filter_token()),
    ];
    let set_var = |vars: &mut Vec<(String, String)>, key: &str, value: &str| {
        if let Some(slot) = vars
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
        {
            slot.1 = value.to_string();
        } else {
            vars.push((key.to_string(), value.to_string()));
        }
    };

    for raw in &lines {
        if let Some((key, value)) = parse_set_line(raw) {
            set_var(&mut vars, &key, &value);
        }
    }

    let mut idx = None;
    for (i, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("::") || lower.starts_with("rem ") || lower.starts_with("@rem ") {
            continue;
        }
        if lower.contains("winws.exe") {
            idx = Some(i);
            break;
        }
    }
    let idx = idx.ok_or_else(|| "В файле нет строки запуска winws.exe".to_string())?;

    let mut cmd = String::new();
    for i in idx..lines.len() {
        let mut line = lines[i].trim().to_string();
        if i == idx {
            let lower = line.to_lowercase();
            if let Some(pos) = lower.find("winws.exe") {
                line = line[pos + "winws.exe".len()..].trim_start().to_string();
                if let Some(rest) = line.strip_prefix('"') {
                    line = rest.to_string();
                }
            }
        }
        let cont = line.ends_with('^');
        if cont {
            line.pop();
        }
        cmd.push_str(&line);
        cmd.push(' ');
        if !cont {
            break;
        }
    }

    let root_with_slash = if inst.root.ends_with('\\') {
        inst.root.clone()
    } else {
        format!("{}\\", inst.root)
    };

    for _ in 0..5 {
        if !cmd.contains('%') {
            break;
        }
        let before = cmd.clone();
        cmd = cmd.replace("%~dp0", &root_with_slash);
        cmd = expand_percent_vars(&cmd, &vars);
        if cmd == before {
            break;
        }
    }

    let argv: Vec<String> = tokenize(&unescape_caret(&cmd))
        .into_iter()
        .filter(|arg| keep_argument(inst, arg))
        .collect();
    if argv.is_empty() {
        return Err("Не удалось разобрать аргументы стратегии".to_string());
    }
    Ok(argv)
}

/// Пустые пользовательские списки не передаём в winws: официальный service.bat
/// создаёт в них заглушки, но при прямом запуске winws отсутствующий/пустой
/// `--hostlist`/`--ipset-exclude` приводит к ошибке запуска. Без аргумента
/// список просто не применяется — это безопасно и позволяет держать файл пустым.
fn keep_argument(inst: &ZapretInstall, arg: &str) -> bool {
    let Some((key, value)) = arg.split_once('=') else {
        return true;
    };
    let key = key.trim_matches('"');
    if !matches!(
        key,
        "--hostlist" | "--hostlist-exclude" | "--ipset" | "--ipset-exclude"
    ) {
        return true;
    }
    let value = value.trim_matches('"');
    let lower = value.to_lowercase();
    if !lower.ends_with("-user.txt") {
        return true;
    }
    let path = if Path::new(value).is_absolute() {
        std::path::PathBuf::from(value)
    } else {
        Path::new(&inst.root).join(value)
    };
    if !path.is_file() {
        return false;
    }
    let text = textfile::read(&path);
    text.lines().any(|line| {
        let line = line.trim();
        !line.is_empty() && !line.starts_with('#')
    })
}

fn parse_set_line(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
    let lower = trimmed.to_lowercase();
    if !lower.starts_with("set ") && !lower.starts_with("set\t") {
        return None;
    }
    let mut rest = trimmed[3..].trim_start();
    if let Some(r) = rest.strip_prefix('"') {
        rest = r;
    }
    let eq = rest.find('=')?;
    let name = rest[..eq].trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '~')
    {
        return None;
    }
    let value = rest[eq + 1..].trim();
    let value = value.strip_suffix('"').unwrap_or(value).trim();
    Some((name.to_string(), value.to_string()))
}

fn expand_percent_vars(cmd: &str, vars: &[(String, String)]) -> String {
    let mut out = String::with_capacity(cmd.len());
    let mut chars = cmd.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // ищем закрывающий %
        let mut name = String::new();
        let mut closed = false;
        let mut consumed = 0usize;
        let mut probe = chars.clone();
        while let Some((_, ch)) = probe.next() {
            consumed += ch.len_utf8();
            if ch == '%' {
                closed = true;
                break;
            }
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '~' {
                name.push(ch);
            } else {
                break;
            }
        }
        if closed && !name.is_empty() {
            if let Some((_, value)) = vars.iter().find(|(k, _)| k.eq_ignore_ascii_case(&name)) {
                out.push_str(value);
                for _ in 0..consumed {
                    chars.next();
                }
                continue;
            }
        }
        out.push('%');
    }
    out
}

/// Снять batch-экранирование `^`.
///
/// В стратегиях встречается `--dpi-desync-fake-tls=^!`: `^!` — это способ
/// передать `!` (встроенный fake TLS) через cmd.exe с delayed expansion.
/// Напрямую winws.exe получает аргумент без `^`.
fn unescape_caret(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '^' {
            if let Some(&next) = chars.peek() {
                if next != '\r' && next != '\n' {
                    out.push(chars.next().unwrap_or(next));
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// Токенизация с сохранением кавычек в токенах (как в C#-версии).
fn tokenize(cmd: &str) -> Vec<String> {
    let mut list = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut has = false;
    for c in cmd.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
            cur.push(c);
            has = true;
        } else if !in_quotes && (c == ' ' || c == '\t' || c == '\r' || c == '\n') {
            if has {
                list.push(cur.clone());
                cur.clear();
                has = false;
            }
        } else {
            cur.push(c);
            has = true;
        }
    }
    if has {
        list.push(cur);
    }
    list
}

pub fn quote(token: &str) -> String {
    if token.is_empty() {
        return "\"\"".to_string();
    }
    if token.contains('"') {
        return token.to_string();
    }
    if !token.contains(' ') && !token.contains('\t') {
        token.to_string()
    } else {
        format!("\"{}\"", token)
    }
}

pub fn build_command_line(argv: &[String]) -> String {
    let mut s = String::new();
    for arg in argv {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&quote(arg));
    }
    s
}

/// Нормализованная строка командной строки для сопоставления стратегий.
pub fn normalize_command_line(s: &str) -> String {
    let replaced: String = s
        .chars()
        .map(|c| match c {
            '"' => ' ',
            '/' => '\\',
            c => c,
        })
        .collect();
    replaced
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_keeps_quotes() {
        let tokens = tokenize("--wf-tcp=80,443 \"C:\\Program Files\\x.txt\" --flag");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[1], "\"C:\\Program Files\\x.txt\"");
        assert_eq!(build_command_line(&tokens), "--wf-tcp=80,443 \"C:\\Program Files\\x.txt\" --flag");
    }

    #[test]
    fn caret_escapes_are_removed() {
        assert_eq!(
            unescape_caret("--dpi-desync-fake-tls=^! --x=^^ --y=^&"),
            "--dpi-desync-fake-tls=! --x=^ --y=&"
        );
    }

    #[test]
    fn fake_tls_bang_survives_strategy_parsing() {
        use super::super::install::{StrategyDef, ZapretInstall};
        let dir = std::env::temp_dir().join(format!("zapret-bat-bang-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("general.bat");
        std::fs::write(
            &file,
            "@echo off\r\nset \"BIN=%~dp0bin\\\\\"\r\n\"%BIN%winws.exe\" --filter-tcp=443 --dpi-desync=fake --dpi-desync-fake-tls=^!\r\n",
        )
        .unwrap();
        let inst = ZapretInstall {
            root: dir.to_string_lossy().to_string(),
            ..Default::default()
        };
        let strategy = StrategyDef {
            file: file.clone(),
            name: "general".to_string(),
            ..Default::default()
        };
        let argv = get_argv(&inst, &strategy, &GameFilterState::default()).unwrap();
        assert!(argv.iter().any(|a| a == "--dpi-desync-fake-tls=!"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_user_list_argument_is_dropped() {
        use super::super::install::ZapretInstall;
        let root = std::env::temp_dir().join(format!(
            "zapret-bat-userlists-{}",
            std::process::id()
        ));
        let lists = root.join("lists");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&lists).unwrap();
        let inst = ZapretInstall {
            root: root.to_string_lossy().to_string(),
            ..Default::default()
        };
        let user = lists.join("list-general-user.txt");
        std::fs::write(&user, "").unwrap();
        let arg = format!("--hostlist=\"{}\"", user.to_string_lossy());
        assert!(!keep_argument(&inst, &arg));
        std::fs::write(&user, "example.com\r\n").unwrap();
        assert!(keep_argument(&inst, &arg));
        // Официальный пустой список не трогаем.
        let official = lists.join("list-general.txt");
        std::fs::write(&official, "").unwrap();
        let official_arg = format!("--hostlist=\"{}\"", official.to_string_lossy());
        assert!(keep_argument(&inst, &official_arg));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn vars_expand() {
        let vars = vec![
            ("LISTS".to_string(), "C:\\zapret\\lists".to_string()),
            ("BIN".to_string(), "C:\\zapret\\bin".to_string()),
        ];
        assert_eq!(
            expand_percent_vars("%BIN%\\winws.exe --hostlist=%LISTS%\\a.txt", &vars),
            "C:\\zapret\\bin\\winws.exe --hostlist=C:\\zapret\\lists\\a.txt"
        );
        assert_eq!(expand_percent_vars("100%%", &vars), "100%%");
    }

    #[test]
    fn parses_real_bat_shape() {
        use super::super::install::{StrategyDef, ZapretInstall};
        let dir = std::env::temp_dir().join(format!("zapret-bat-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("general.bat");
        std::fs::write(
            &file,
            "@echo off\r\
set \"BIN=%~dp0bin\\\"\r\
set \"LISTS=%~dp0lists\\\"\r\
\"%BIN%winws.exe\" --wf-tcp=%GameFilterTCP% ^\r\
 --hostlist=\"%LISTS%list-general.txt\" ^\r\
 --wf-udp=%GameFilterUDP%\r\
",
        )
        .unwrap();
        let inst = ZapretInstall {
            root: dir.to_string_lossy().to_string(),
            ..Default::default()
        };
        let strategy = StrategyDef {
            file: file.clone(),
            name: "general".to_string(),
            ..Default::default()
        };
        let gf = crate::engine::gamefilter::GameFilterState {
            mode: crate::engine::gamefilter::GameFilterMode::All,
            tcp_range: "80,443".to_string(),
            udp_range: "50000-50100".to_string(),
        };
        let argv = get_argv(&inst, &strategy, &gf).expect("argv");
        assert!(argv.iter().any(|a| a.contains("--wf-tcp=80,443")));
        assert!(argv.iter().any(|a| a.contains("list-general.txt")));
        assert!(argv.iter().any(|a| a.contains("--wf-tcp=80,443")));
        assert!(argv.iter().any(|a| a.contains("--wf-udp=50000-50100")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
