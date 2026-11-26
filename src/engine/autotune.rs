//! Автоподбор стратегии: варианты desync-аргументов на основе существующих .bat.
//!
//! Модуль только готовит кандидатов (пишет временные .bat) и сохраняет лучшего.
//! Сам прогон выполняет `tester::run_candidates`.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::bat;
use super::config::Ini;
use super::gamefilter::GameFilterState;
use super::install::{StrategyDef, StrategyKind, ZapretInstall};
use super::log;
use super::ops::ZapretOps;
use super::paths;
use super::service;
use super::tester::{self, StrategyTestRow, TesterOptions};
use super::textfile;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub file: PathBuf,
}

const TUNABLE: [&str; 21] = [
    "--dpi-desync",
    "--dpi-desync-repeats",
    "--dpi-desync-split-pos",
    "--dpi-desync-split-seqovl",
    "--dpi-desync-split-seqovl-pattern",
    "--dpi-desync-fakedsplit-pattern",
    "--dpi-desync-fooling",
    "--dpi-desync-autottl",
    "--dpi-desync-ttl",
    "--dpi-desync-cutoff",
    "--dpi-desync-badseq-increment",
    "--dpi-desync-hostfakesplit-mod",
    "--dpi-desync-hostfakesplit-midhost",
    "--dpi-desync-fake-tls-mod",
    "--dpi-desync-fake-tls",
    "--dpi-desync-fake-http",
    "--dpi-desync-fake-quic",
    "--dpi-desync-fake-unknown-udp",
    "--dpi-desync-fake-unknown",
    "--dpi-desync-fake-discord",
    "--dpi-desync-fake-stun",
];

#[derive(Default)]
struct Pools {
    generic: HashMap<String, Vec<String>>,
    by_proto: HashMap<(String, String), Vec<String>>,
}

impl Pools {
    fn add(&mut self, key: &str, proto: &str, value: &str) {
        if value.is_empty() {
            return;
        }
        add_unique(self.generic.entry(key.to_string()).or_default(), value);
        add_unique(
            self.by_proto
                .entry((key.to_string(), proto.to_string()))
                .or_default(),
            value,
        );
    }

    fn values(&self, key: &str, proto: &str) -> Vec<String> {
        let mut out = self
            .by_proto
            .get(&(key.to_string(), proto.to_string()))
            .cloned()
            .unwrap_or_default();
        if out.is_empty() {
            out = self.generic.get(key).cloned().unwrap_or_default();
        }
        out
    }
}

fn add_unique(list: &mut Vec<String>, value: &str) {
    if !list.iter().any(|v| v.eq_ignore_ascii_case(value)) {
        list.push(value.to_string());
    }
}

fn split_arg(token: &str) -> Option<(&str, &str)> {
    let (key, value) = token.split_once('=')?;
    Some((key.trim_matches('"'), value.trim_matches('"')))
}

fn tunable_key(key: &str) -> bool {
    TUNABLE.iter().any(|k| k.eq_ignore_ascii_case(key))
}

fn split_sections(argv: &[String]) -> Vec<Vec<String>> {
    let mut sections: Vec<Vec<String>> = vec![Vec::new()];
    for token in argv {
        if token.trim_matches('"').eq_ignore_ascii_case("--new") {
            sections.push(Vec::new());
        } else if let Some(last) = sections.last_mut() {
            last.push(token.clone());
        }
    }
    sections.retain(|s| !s.is_empty());
    sections
}

fn join_sections(sections: &[Vec<String>]) -> Vec<String> {
    let mut out = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            out.push("--new".to_string());
        }
        out.extend(section.iter().cloned());
    }
    out
}

fn section_proto(section: &[String]) -> String {
    for token in section {
        let key = token
            .split_once('=')
            .map(|(k, _)| k)
            .unwrap_or(token)
            .trim_matches('"')
            .to_ascii_lowercase();
        if key == "--filter-udp" {
            return "udp".to_string();
        }
        if key == "--filter-tcp" {
            return "tcp".to_string();
        }
    }
    "any".to_string()
}

fn collect_pools(install: &ZapretInstall, gf: &GameFilterState) -> Pools {
    let mut pools = Pools::default();
    for strategy in &install.strategies {
        let Ok(argv) = bat::get_argv(install, strategy, gf) else {
            continue;
        };
        let sections = split_sections(&argv);
        for section in sections {
            let proto = section_proto(&section);
            for token in &section {
                let Some((key, value)) = split_arg(token) else {
                    continue;
                };
                if tunable_key(key) {
                    pools.add(key, &proto, value);
                }
            }
        }
    }

    // Значения, которых может не быть в текущих .bat, но которые полезно
    // проверить в долгом автоподборе.
    let common: &[(&str, &[&str])] = &[
        ("--dpi-desync-repeats", &["2", "4", "6", "8", "11", "12", "16"]),
        (
            "--dpi-desync-split-pos",
            &["1", "2", "3", "4", "5", "8", "10", "1,2", "2,3", "sni"],
        ),
        (
            "--dpi-desync-split-seqovl",
            &["0", "1", "2", "568", "582", "652", "654", "681", "726", "1460"],
        ),
        (
            "--dpi-desync-fooling",
            &[
                "md5sig",
                "badseq",
                "badsum",
                "ts",
                "md5sig,badseq",
                "badsum,badseq",
                "badseq,badsum",
                "ts,md5sig",
                "hopbyhop",
            ],
        ),
        ("--dpi-desync-autottl", &["1", "2", "3", "4", "5", "8"]),
        ("--dpi-desync-ttl", &["1", "2", "3", "4", "5", "8", "16", "32"]),
        ("--dpi-desync-cutoff", &["d2", "n2", "n5"]),
        ("--dpi-desync-badseq-increment", &["0"]),
    ];
    for (key, values) in common {
        for value in *values {
            for proto in ["tcp", "udp", "any"] {
                pools.add(key, proto, value);
            }
        }
    }
    let tcp_only: &[(&str, &[&str])] = &[
        (
            "--dpi-desync",
            &[
                "fake",
                "fake,split2",
                "fake,multisplit",
                "fake,fakeddisorder",
                "split2",
                "multisplit",
                "fakedsplit",
                "hostfakesplit",
            ],
        ),
        (
            "--dpi-desync-hostfakesplit-mod",
            &[
                "host=google.com",
                "host=ozon.ru",
                "host=i2.photo.2gis.com",
                "host=maxcdn.bootstrapcdn.com",
            ],
        ),
        ("--dpi-desync-hostfakesplit-midhost", &["host-2"]),
        (
            "--dpi-desync-fake-tls-mod",
            &[
                "rnd,dupsid,sni=www.google.com",
                "rnd,sni=maxcdn.bootstrapcdn.com",
                "none",
            ],
        ),
    ];
    for (key, values) in tcp_only {
        for value in *values {
            for proto in ["tcp", "any"] {
                pools.add(key, proto, value);
            }
        }
    }
    for value in ["fake", "fake,split2", "fake,multisplit"] {
        for proto in ["udp", "any"] {
            pools.add("--dpi-desync", proto, value);
        }
    }
    // Файлы, которые уже лежат в bin и подходят для blob-аргументов.
    let bin = install.bin_dir();
    if let Some(path) = blob_if_exists(&bin, "stun.bin") {
        for key in [
            "--dpi-desync-fakedsplit-pattern",
            "--dpi-desync-split-seqovl-pattern",
            "--dpi-desync-fake-stun",
        ] {
            for proto in ["tcp", "udp", "any"] {
                pools.add(key, proto, &path);
            }
        }
    }
    if let Some(path) = blob_if_exists(&bin, "ACTIVE_DISCORD_UDP.bin") {
        for proto in ["udp", "any"] {
            pools.add("--dpi-desync-fake-discord", proto, &path);
        }
    }
    if let Some(path) = blob_if_exists(&bin, "ACTIVE_GAME_UDP.bin") {
        for proto in ["udp", "any"] {
            pools.add("--dpi-desync-fake-unknown-udp", proto, &path);
        }
    }
    pools
}

fn blob_if_exists(dir: &Path, name: &str) -> Option<String> {
    let path = dir.join(name);
    if path.is_file() {
        Some(path.to_string_lossy().to_string())
    } else {
        None
    }
}

fn short_key(key: &str) -> String {
    key.trim_start_matches("--")
        .trim_start_matches("dpi-desync-")
        .to_string()
}

fn short_value(key: &str, value: &str) -> String {
    if key.starts_with("--dpi-desync-fake") {
        return Path::new(value)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| value.to_string());
    }
    crate::util::shorten(value, 24)
}

fn replace_key(section: &mut Vec<String>, key: &str, value: &str) -> bool {
    for token in section.iter_mut() {
        if let Some((k, _)) = token.split_once('=') {
            if k.trim_matches('"').eq_ignore_ascii_case(key) {
                *token = format!("{}={}", key, value);
                return true;
            }
        }
    }
    // Ключа нет — добавляем сразу после --dpi-desync, чтобы можно было
    // проверить repeats/fooling/ttl, которых нет в базовой стратегии.
    if let Some(pos) = section.iter().position(|token| {
        token
            .split_once('=')
            .map(|(k, _)| k.trim_matches('"').eq_ignore_ascii_case("--dpi-desync"))
            .unwrap_or(false)
    }) {
        section.insert(pos + 1, format!("{}={}", key, value));
        return true;
    }
    false
}

fn section_candidates(section: &[String], pools: &Pools) -> Vec<(String, Vec<(String, String)>)> {
    let proto = section_proto(section);
    let mut keys: Vec<(String, String)> = Vec::new();
    for token in section {
        let Some((key, value)) = split_arg(token) else {
            continue;
        };
        if tunable_key(key) && !keys.iter().any(|(k, _)| k.eq_ignore_ascii_case(key)) {
            keys.push((key.to_string(), value.to_string()));
        }
    }
    let has_desync = keys
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("--dpi-desync"));
    if has_desync {
        for key in [
            "--dpi-desync-repeats",
            "--dpi-desync-fooling",
            "--dpi-desync-autottl",
            "--dpi-desync-ttl",
        ] {
            if !keys.iter().any(|(k, _)| k.eq_ignore_ascii_case(key)) {
                keys.push((key.to_string(), String::new()));
            }
        }
        let fake_key = if proto == "udp" {
            "--dpi-desync-fake-quic"
        } else {
            "--dpi-desync-fake-tls"
        };
        if !keys.iter().any(|(k, _)| k.eq_ignore_ascii_case(fake_key)) {
            keys.push((fake_key.to_string(), String::new()));
        }
        for key in [
            "--dpi-desync-cutoff",
            "--dpi-desync-split-seqovl-pattern",
        ] {
            if !keys.iter().any(|(k, _)| k.eq_ignore_ascii_case(key)) {
                keys.push((key.to_string(), String::new()));
            }
        }
        if proto == "tcp" {
            for key in [
                "--dpi-desync-hostfakesplit-mod",
                "--dpi-desync-hostfakesplit-midhost",
                "--dpi-desync-fake-tls-mod",
                "--dpi-desync-badseq-increment",
                "--dpi-desync-fakedsplit-pattern",
            ] {
                if !keys.iter().any(|(k, _)| k.eq_ignore_ascii_case(key)) {
                    keys.push((key.to_string(), String::new()));
                }
            }
        } else {
            for key in ["--dpi-desync-fake-discord", "--dpi-desync-fake-stun"] {
                if !keys.iter().any(|(k, _)| k.eq_ignore_ascii_case(key)) {
                    keys.push((key.to_string(), String::new()));
                }
            }
        }
    }
    let mut out = Vec::new();
    for (key, _) in &keys {
        for value in pools.values(key, &proto) {
            out.push((
                format!("{}={}", short_key(key), short_value(key, &value)),
                vec![(key.clone(), value)],
            ));
        }
    }
    // Пары, которые чаще всего влияют на ускорение.
    let pair_names = [
        ("--dpi-desync-repeats", "--dpi-desync-split-pos"),
        ("--dpi-desync-split-seqovl", "--dpi-desync-split-pos"),
        ("--dpi-desync", "--dpi-desync-repeats"),
        ("--dpi-desync", "--dpi-desync-fooling"),
        ("--dpi-desync-fake-tls", "--dpi-desync-repeats"),
        ("--dpi-desync-fake-quic", "--dpi-desync-repeats"),
        ("--dpi-desync", "--dpi-desync-hostfakesplit-mod"),
        ("--dpi-desync", "--dpi-desync-fake-tls-mod"),
        ("--dpi-desync-hostfakesplit-mod", "--dpi-desync-fooling"),
        ("--dpi-desync-badseq-increment", "--dpi-desync-fooling"),
    ];
    for (a, b) in pair_names {
        let Some((_, av)) = keys.iter().find(|(k, _)| k.eq_ignore_ascii_case(a)) else {
            continue;
        };
        let Some((_, bv)) = keys.iter().find(|(k, _)| k.eq_ignore_ascii_case(b)) else {
            continue;
        };
        for va in pools.values(a, &proto) {
            if va.eq_ignore_ascii_case(av) {
                continue;
            }
            for vb in pools.values(b, &proto) {
                if vb.eq_ignore_ascii_case(bv) {
                    continue;
                }
                if out.len() > 500 {
                    return out;
                }
                out.push((
                    format!(
                        "{}={}; {}={}",
                        short_key(a),
                        short_value(a, &va),
                        short_key(b),
                        short_value(b, &vb)
                    ),
                    vec![(a.to_string(), va.clone()), (b.to_string(), vb)],
                ));
            }
        }
    }
    out
}

/// Подготовить до `limit` вариантов вокруг `base` и записать их во временные .bat.
pub fn build_plan(
    install: &ZapretInstall,
    base_name: &str,
    limit: usize,
) -> Result<Vec<Candidate>, String> {
    let limit = limit.clamp(5, 400);
    let base = install
        .find_strategy(base_name)
        .cloned()
        .or_else(|| install.default_strategy().cloned())
        .ok_or_else(|| "Стратегии не найдены.".to_string())?;
    let gf = GameFilterState::load(&install.root);
    let base_argv = bat::get_argv(install, &base, &gf)?;
    let sections = split_sections(&base_argv);
    if sections.is_empty() {
        return Err("Не удалось разобрать базовую стратегию.".to_string());
    }
    let pools = collect_pools(install, &gf);

    let mut variants: Vec<(String, Vec<String>)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let base_key = base_argv.join("\u{1}");
    seen.insert(base_key);

    let mut push = |label: String, sections_next: Vec<Vec<String>>, variants: &mut Vec<(String, Vec<String>)>| {
        let argv = join_sections(&sections_next);
        let key = argv.join("\u{1}");
        if seen.insert(key) {
            variants.push((label, argv));
        }
    };

    // Одиночные замены во всех секциях.
    for (si, section) in sections.iter().enumerate() {
        for (label, changes) in section_candidates(section, &pools) {
            if variants.len() + 1 >= limit {
                break;
            }
            let mut next = sections.clone();
            let mut ok = true;
            for (key, value) in &changes {
                if !replace_key(&mut next[si], key, value) {
                    ok = false;
                    break;
                }
            }
            if ok {
                push(label, next, &mut variants);
            }
        }
        if variants.len() + 1 >= limit {
            break;
        }
    }

    // Если одиночных не хватило — комбинируем изменения из разных секций
    // детерминированным перебором.
    if variants.len() + 1 < limit {
        let mut all_changes: Vec<(usize, String, Vec<(String, String)>)> = Vec::new();
        for (si, section) in sections.iter().enumerate() {
            for (label, changes) in section_candidates(section, &pools).into_iter().take(24) {
                all_changes.push((si, label, changes));
            }
        }
        let mut i = 0usize;
        while variants.len() + 1 < limit && i < all_changes.len() {
            let (si1, label1, ch1) = all_changes[i].clone();
            let j = (i * 7 + 3) % all_changes.len().max(1);
            let (si2, label2, ch2) = all_changes[j].clone();
            if si1 != si2 {
                let mut next = sections.clone();
                let mut ok = true;
                for (key, value) in ch1.iter().chain(ch2.iter()) {
                    if !replace_key(&mut next[si1], key, value)
                        && !replace_key(&mut next[si2], key, value)
                    {
                        ok = false;
                    }
                }
                if ok {
                    push(
                        format!("{} + {}", label1, label2),
                        next,
                        &mut variants,
                    );
                }
            }
            i += 1;
        }
    }

    let dir = paths::temp_dir().join(format!("autotune-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    let base_file = dir.join("00-base.bat");
    write_bat(&base_file, &base_argv)?;
    out.push(Candidate {
        name: format!("{} (база)", base.name),
        file: base_file,
    });
    for (i, (label, argv)) in variants.into_iter().take(limit.saturating_sub(1)).enumerate() {
        let file = dir.join(format!("{:02}-auto.bat", i + 1));
        write_bat(&file, &argv)?;
        out.push(Candidate {
            name: format!("A{:02} {}", i + 1, label),
            file,
        });
    }
    Ok(out)
}

fn write_bat(path: &Path, argv: &[String]) -> Result<(), String> {
    let cmd = bat::build_command_line(argv);
    let content = format!(
        "@echo off\r\ncd /d \"%~dp0\"\r\n\"%~dp0bin\\winws.exe\" {}\r\n",
        cmd
    );
    textfile::write_text(path, &content).map_err(|e| e.to_string())
}

/// Результат автоподбора.
#[derive(Debug, Clone, Default)]
pub struct AutoReport {
    pub rows: Vec<StrategyTestRow>,
    pub best: Option<StrategyTestRow>,
    pub best_file: Option<PathBuf>,
    pub candidates: usize,
}

/// Сгенерировать варианты и прогнать их через обычный тест стратегий.
#[allow(clippy::too_many_arguments)]
pub fn run_search(
    install: &ZapretInstall,
    base_name: &str,
    limit: usize,
    selected_groups: &[String],
    options: &TesterOptions,
    mut progress: impl FnMut(usize, usize, &str),
    mut row_done: impl FnMut(&StrategyTestRow),
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<AutoReport, String> {
    let plan = build_plan(install, base_name, limit)?;
    let candidates = plan.len();
    let strategies: Vec<StrategyDef> = plan
        .iter()
        .map(|c| StrategyDef {
            file: c.file.clone(),
            name: c.name.clone(),
            short_name: c.name.clone(),
            kind: StrategyKind::Other,
            description: "Автоподбор".to_string(),
            warning: false,
            order: 0,
        })
        .collect();
    let mut rows = tester::run_candidates(
        install,
        &strategies,
        selected_groups,
        options,
        &mut progress,
        &mut row_done,
        cancelled,
    );
    tester::sort_rows(&mut rows);
    let best = rows
        .iter()
        .find(|r| r.best_candidate())
        .cloned()
        .filter(|r| r.passed() > 0);
    let best_file = best.as_ref().and_then(|best| {
        plan.iter()
            .find(|c| c.name == best.strategy_name)
            .and_then(|c| save_best(install, &c.file).ok())
    });
    let _ = std::fs::remove_dir_all(
        paths::temp_dir().join(format!("autotune-{}", std::process::id())),
    );
    Ok(AutoReport {
        rows,
        best,
        best_file,
        candidates,
    })
}

/// Сохранить лучший вариант в корень zapret как обычную стратегию.
pub fn save_best(install: &ZapretInstall, file: &Path) -> Result<PathBuf, String> {
    if !file.is_file() {
        return Err("Файл лучшего варианта не найден.".to_string());
    }
    let root = Path::new(&install.root);
    for i in 1..100 {
        let name = if i == 1 {
            "general (AUTO).bat".to_string()
        } else {
            format!("general (AUTO {}).bat", i)
        };
        let target = root.join(name);
        if !target.exists() {
            std::fs::copy(file, &target).map_err(|e| e.to_string())?;
            return Ok(target);
        }
    }
    Err("Не удалось подобрать имя для auto-стратегии.".to_string())
}

#[derive(Debug, Clone)]
pub struct ApplyReport {
    pub applied: bool,
    pub message: String,
    pub row: StrategyTestRow,
    pub best_passed: usize,
    pub best_total: usize,
}

fn restore_previous(
    ops: &ZapretOps,
    previous: Option<&StrategyDef>,
    service_was_running: bool,
    was_running: bool,
) {
    if !was_running {
        ops.kill_winws();
        return;
    }
    if service_was_running {
        if let Err(e) = service::start("zapret") {
            log::warn(format!("Не удалось вернуть службу zapret: {}", e));
        }
        return;
    }
    if let Some(prev) = previous {
        let result = ops.start(prev);
        if !result.ok {
            log::warn(format!("Не удалось вернуть стратегию: {}", result.message));
        }
    }
}

/// Применить лучший auto-файл, проверить цели и откатиться при ухудшении.
#[allow(clippy::too_many_arguments)]
pub fn apply_and_verify(
    install: &ZapretInstall,
    file: &Path,
    best_passed: usize,
    selected_groups: &[String],
    options: &TesterOptions,
    previous: Option<StrategyDef>,
) -> Result<ApplyReport, String> {
    if !file.is_file() {
        return Err("Файл auto-стратегии не найден.".to_string());
    }
    let name = file
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "general (AUTO)".to_string());
    let best = StrategyDef {
        file: file.to_path_buf(),
        name: name.clone(),
        short_name: "AUTO".to_string(),
        kind: StrategyKind::Other,
        description: "Автоподбор".to_string(),
        warning: false,
        order: 0,
    };
    let ops = ZapretOps::new(install.clone());
    let service_snap = service::query("zapret");
    let service_was_running = service_snap.installed && service_snap.status == "работает";
    let was_running = ops.get_state().winws_running || service_was_running;

    if service_was_running {
        let _ = service::stop("zapret");
        std::thread::sleep(std::time::Duration::from_millis(1200));
    }
    ops.kill_winws();
    std::thread::sleep(std::time::Duration::from_millis(300));

    let start = ops.start(&best);
    if !start.ok {
        restore_previous(&ops, previous.as_ref(), service_was_running, was_running);
        return Err(format!("Не удалось запустить auto-стратегию: {}", start.message));
    }
    std::thread::sleep(std::time::Duration::from_millis(800));
    let row = tester::verify_current(install, selected_groups, options);
    let passed = row.passed();
    let total = row.total();
    let acceptable = total > 0 && passed > 0 && passed + 2 >= best_passed;
    if acceptable {
        let mut ini = Ini::load(&paths::config_path());
        ini.set("LastStrategy", &name);
        let _ = ini.save();
        let message = format!(
            "Стратегия «{}» применена и проверена: {}/{} целей.",
            name, passed, total
        );
        log::ok(&message);
        return Ok(ApplyReport {
            applied: true,
            message,
            row,
            best_passed,
            best_total: total,
        });
    }

    let mut msg = format!(
        "Проверка после применения: {}/{} целей (в автоподборе было {}/{}). Откатываю.",
        passed,
        total,
        best_passed,
        row.total()
    );
    if !row.error.is_empty() {
        msg.push_str(&format!(" Ошибка: {}", row.error));
    }
    log::warn(&msg);
    let _ = ops.stop();
    std::thread::sleep(std::time::Duration::from_millis(600));
    restore_previous(&ops, previous.as_ref(), service_was_running, was_running);
    Ok(ApplyReport {
        applied: false,
        message: msg,
        row,
        best_passed,
        best_total: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::install::StrategyDef;

    #[test]
    fn plan_generates_parseable_variants() {
        let root = std::env::temp_dir().join(format!(
            "zapret-autotune-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("general.bat");
        std::fs::write(
            &file,
            "@echo off\r\n\
             set \"BIN=%~dp0bin\\\\\"\r\n\
             set \"LISTS=%~dp0lists\\\\\"\r\n\
             \"%BIN%winws.exe\" --filter-tcp=443 --dpi-desync=multisplit ^\r\n\
              --dpi-desync-split-pos=1 --dpi-desync-repeats=6 --dpi-desync-split-seqovl=568\r\n",
        )
        .unwrap();
        let inst = ZapretInstall {
            root: root.to_string_lossy().to_string(),
            strategies: vec![StrategyDef {
                file: file.clone(),
                name: "general".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let plan = build_plan(&inst, "general", 120).unwrap();
        assert!(plan.len() > 1);
        assert!(plan
            .iter()
            .any(|c| c.name.to_lowercase().contains("hostfakesplit")));
        assert!(plan
            .iter()
            .any(|c| c.name.to_lowercase().contains("fake-tls-mod")));
        let gf = GameFilterState::load(&inst.root);
        for candidate in &plan {
            assert!(candidate.file.is_file());
            let strategy = StrategyDef {
                file: candidate.file.clone(),
                name: candidate.name.clone(),
                ..Default::default()
            };
            let argv = bat::get_argv(&inst, &strategy, &gf).unwrap();
            assert!(argv.iter().any(|a| a.starts_with("--filter-tcp")));
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
