//! Тест стратегий: прогон winws.exe по целям из utils\targets.txt.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::bat;
use super::gamefilter::GameFilterState;
use super::http;
use super::install::{StrategyDef, ZapretInstall};
use super::log;
use super::netcheck;
use super::ops::ZapretOps;
use super::process;
use super::service;
use super::textfile;

#[derive(Debug, Clone, Default)]
pub struct TestTarget {
    pub name: String,
    pub url: String,
    pub ping_host: String,
    pub is_ping: bool,
    pub baseline_ok: bool,
}

impl TestTarget {
    pub fn display(&self) -> String {
        if self.is_ping {
            format!("ping {}", self.ping_host)
        } else {
            self.url.clone()
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct TargetGroup {
    pub name: String,
    pub targets: Vec<TestTarget>,
}

/// Разбор utils\targets.txt.
pub fn load_targets(path: &Path) -> Vec<TargetGroup> {
    let mut groups: Vec<TargetGroup> = Vec::new();
    let mut current: Option<TargetGroup> = None;
    for raw in textfile::read_lines(path) {
        let line = raw.trim().to_string();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("###") {
            if let Some(g) = current.take() {
                if !g.targets.is_empty() {
                    groups.push(g);
                }
            }
            let name = rest.trim_start_matches('#').trim().to_string();
            current = Some(TargetGroup {
                name: if name.is_empty() {
                    "Прочее".to_string()
                } else {
                    name
                },
                targets: Vec::new(),
            });
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let Some(eq) = line.find('=') else { continue };
        let key = line[..eq].trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            continue;
        }
        let value = line[eq + 1..].trim();
        let Some(value) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
            continue;
        };
        let value = value.trim();
        if value.to_lowercase().starts_with("ping:") {
            let host = value[5..].trim().to_string();
            if host.is_empty() {
                continue;
            }
            let group = current.get_or_insert_with(|| TargetGroup {
                name: "Прочее".to_string(),
                targets: Vec::new(),
            });
            group.targets.push(TestTarget {
                name: key.to_string(),
                ping_host: host,
                is_ping: true,
                ..Default::default()
            });
        } else {
            if value.is_empty() {
                continue;
            }
            let host = value
                .to_lowercase()
                .trim_start_matches("https://")
                .trim_start_matches("http://")
                .split('/')
                .next()
                .unwrap_or("")
                .to_string();
            if host.is_empty() {
                continue;
            }
            let group = current.get_or_insert_with(|| TargetGroup {
                name: "Прочее".to_string(),
                targets: Vec::new(),
            });
            group.targets.push(TestTarget {
                name: key.to_string(),
                url: value.to_string(),
                ping_host: host,
                is_ping: false,
                ..Default::default()
            });
        }
    }
    if let Some(g) = current {
        if !g.targets.is_empty() {
            groups.push(g);
        }
    }
    groups
}

#[derive(Debug, Clone, Default)]
pub struct GroupResult {
    pub group: String,
    pub passed: usize,
    pub total: usize,
    pub ms: u64,
}

impl GroupResult {
    pub fn ok(&self) -> bool {
        self.total > 0 && self.passed == self.total
    }
    pub fn partial(&self) -> bool {
        self.passed > 0 && self.passed < self.total
    }
    /// Сервис считается рабочим, если прошло не меньше 60% его целей.
    pub fn healthy(&self) -> bool {
        self.total > 0 && self.passed * 100 >= self.total * 60
    }
    pub fn cell_text(&self) -> String {
        if self.total == 0 {
            return "—".to_string();
        }
        if self.passed == 0 {
            return "нет".to_string();
        }
        if self.passed == self.total {
            if self.ms > 0 {
                format!("да {} мс", self.ms)
            } else {
                "да".to_string()
            }
        } else {
            format!("{}/{}", self.passed, self.total)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct StrategyTestRow {
    pub strategy_name: String,
    pub strategy_file: String,
    pub groups: Vec<GroupResult>,
    pub error: String,
}

impl StrategyTestRow {
    pub fn passed(&self) -> usize {
        self.groups.iter().map(|g| g.passed).sum()
    }
    pub fn total(&self) -> usize {
        self.groups.iter().map(|g| g.total).sum()
    }
    pub fn all_ok(&self) -> bool {
        self.total() > 0 && self.passed() == self.total()
    }
    pub fn healthy_groups(&self) -> usize {
        self.groups.iter().filter(|g| g.healthy()).count()
    }
    pub fn score(&self) -> (usize, usize) {
        (self.healthy_groups(), self.passed())
    }
    pub fn best_candidate(&self) -> bool {
        self.error.is_empty() && self.total() > 0
    }
}

/// Отсортировать результаты от лучшего к худшему.
///
/// Сначала идут стратегии с наибольшим числом рабочих групп, затем — с
/// наибольшим числом прошедших целей. Не запустившиеся стратегии уходят вниз.
pub fn sort_rows(rows: &mut [StrategyTestRow]) {
    rows.sort_by(|a, b| {
        let a_ok = a.best_candidate();
        let b_ok = b.best_candidate();
        b_ok.cmp(&a_ok)
            .then_with(|| b.score().cmp(&a.score()))
            .then_with(|| b.total().cmp(&a.total()))
            .then_with(|| a.strategy_name.cmp(&b.strategy_name))
    });
}

#[derive(Debug, Clone)]
pub struct TesterOptions {
    pub timeout_ms: u32,
    pub deep_check: bool,
    pub restore_after: bool,
}

impl Default for TesterOptions {
    fn default() -> Self {
        TesterOptions {
            timeout_ms: 4000,
            deep_check: false,
            restore_after: true,
        }
    }
}

fn looks_like_block_page(text: &str) -> bool {
    let lower = text.to_lowercase();
    const MARKERS: [&str; 7] = [
        "доступ ограничен",
        "доступ к запрашиваемому",
        "заблокирован",
        "blockpage",
        "blocked by",
        "zapret-info",
        "роскомнадзор",
    ];
    MARKERS.iter().any(|m| lower.contains(m))
}

/// HTTP-проверка цели.
fn check_http(url: &str, timeout_ms: u32, deep: bool) -> (bool, u64, String) {
    let start = Instant::now();
    let mut headers: Vec<(&str, &str)> = vec![(
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36",
    )];
    if deep {
        headers.push(("Range", "bytes=0-32767"));
    }
    match http::request(url, "GET", &headers, None, timeout_ms) {
        Ok(resp) => {
            let ms = start.elapsed().as_millis() as u64;
            if resp.status >= 500 || resp.status == 0 {
                return (false, ms, format!("HTTP {}", resp.status));
            }
            if deep && resp.body.len() < 16000 {
                return (
                    false,
                    ms,
                    format!("получено всего {} Б", resp.body.len()),
                );
            }
            let text = resp.text();
            let cp = super::textfile::decode_cp1251(&resp.body);
            if looks_like_block_page(&text) || looks_like_block_page(&cp) {
                return (false, ms, "страница-заглушка провайдера".to_string());
            }
            (true, ms, String::new())
        }
        Err(e) => (false, start.elapsed().as_millis() as u64, crate::util::shorten(&e, 60)),
    }
}

fn check_target(target: &TestTarget, timeout_ms: u32, deep: bool) -> (bool, u64, String) {
    if target.is_ping {
        let (ok, ms, err) = netcheck::ping(&target.ping_host, timeout_ms);
        (ok, ms, err)
    } else {
        check_http(&target.url, timeout_ms, deep)
    }
}

/// Проверить цели на уже запущенном ускорении, без остановки и baseline.
/// Нужно автоподбору после применения лучшей стратегии.
pub fn verify_current(
    install: &ZapretInstall,
    selected_groups: &[String],
    options: &TesterOptions,
) -> StrategyTestRow {
    let all_groups = load_targets(&install.utils_dir().join("targets.txt"));
    let groups: Vec<TargetGroup> = all_groups
        .into_iter()
        .filter(|g| selected_groups.iter().any(|s| s.eq_ignore_ascii_case(&g.name)))
        .collect();
    let mut row = StrategyTestRow {
        strategy_name: "Текущее ускорение".to_string(),
        ..Default::default()
    };
    if groups.is_empty() {
        row.error = "Не выбраны цели для проверки.".to_string();
        return row;
    }
    for group in &groups {
        let mut passed = 0;
        let mut ms = 0;
        for target in &group.targets {
            let (ok, elapsed, _) = check_target(target, options.timeout_ms, options.deep_check);
            if ok {
                passed += 1;
            }
            ms = ms.max(elapsed);
        }
        row.groups.push(GroupResult {
            group: group.name.clone(),
            passed,
            total: group.targets.len(),
            ms,
        });
    }
    row
}

/// Запустить последовательный прогон стратегий.
#[allow(clippy::too_many_arguments)]
pub fn run(
    install: &ZapretInstall,
    selected_groups: &[String],
    options: &TesterOptions,
    progress: impl FnMut(usize, usize, &str),
    row_done: impl FnMut(&StrategyTestRow),
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Vec<StrategyTestRow> {
    run_candidates(
        install,
        &install.strategies,
        selected_groups,
        options,
        progress,
        row_done,
        cancelled,
    )
}

/// Прогон произвольного набора стратегий (нужен автоподбору).
#[allow(clippy::too_many_arguments)]
pub fn run_candidates(
    install: &ZapretInstall,
    strategies: &[StrategyDef],
    selected_groups: &[String],
    options: &TesterOptions,
    mut progress: impl FnMut(usize, usize, &str),
    mut row_done: impl FnMut(&StrategyTestRow),
    cancelled: &(dyn Fn() -> bool + Sync),
) -> Vec<StrategyTestRow> {
    let all_groups = load_targets(&install.utils_dir().join("targets.txt"));
    install.ensure_user_lists();
    let groups: Vec<TargetGroup> = all_groups
        .into_iter()
        .filter(|g| selected_groups.iter().any(|s| s.eq_ignore_ascii_case(&g.name)))
        .collect();
    if groups.is_empty() || strategies.is_empty() {
        return Vec::new();
    }

    let ops = ZapretOps::new(install.clone());
    let before = ops.get_state();
    let snap = service::query("zapret");
    let service_was_running = snap.installed && snap.status == "работает";

    if service_was_running {
        log::info("Тест: останавливаю службу zapret на время прогона.");
        let _ = service::stop("zapret");
        std::thread::sleep(Duration::from_millis(1200));
    }
    ops.kill_winws();
    std::thread::sleep(Duration::from_millis(300));

    // Базовая проверка: цели, доступные без ускорения, из оценки исключаются.
    let mut baseline_targets: Vec<TestTarget> = groups.iter().flat_map(|g| g.targets.clone()).collect();
    let baseline_results: Vec<bool> = {
        let index = std::sync::atomic::AtomicUsize::new(0);
        let results = std::sync::Mutex::new(vec![false; baseline_targets.len()]);
        std::thread::scope(|scope| {
            for _ in 0..6.min(baseline_targets.len().max(1)) {
                let index = &index;
                let targets = &baseline_targets;
                let results = &results;
                scope.spawn(move || loop {
                    let i = index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if i >= targets.len() {
                        break;
                    }
                    let (ok, _, _) = check_target(&targets[i], options.timeout_ms, false);
                    if let Ok(mut list) = results.lock() {
                        list[i] = ok;
                    }
                });
            }
        });
        results.into_inner().unwrap_or_default()
    };
    for (target, ok) in baseline_targets.iter_mut().zip(baseline_results.iter()) {
        target.baseline_ok = *ok;
    }
    let baseline_reachable = baseline_results.iter().filter(|ok| **ok).count();
    let _ = &baseline_targets;
    if baseline_reachable > 0 {
        log::warn(format!(
            "Тест: без ускорения доступно целей {} — они исключены из оценки стратегий.",
            baseline_reachable
        ));
    }

    let mut rows: Vec<StrategyTestRow> = Vec::new();
    let total_strategies = strategies.len();
    for (i, strategy) in strategies.iter().enumerate() {
        if cancelled() {
            break;
        }
        progress(i + 1, total_strategies, &strategy.name);
        let mut row = StrategyTestRow {
            strategy_name: strategy.name.clone(),
            strategy_file: strategy.file.to_string_lossy().to_string(),
            ..Default::default()
        };
        for g in &groups {
            let total = g.targets.iter().filter(|t| !t.baseline_ok).count();
            row.groups.push(GroupResult {
                group: g.name.clone(),
                passed: 0,
                total,
                ms: 0,
            });
        }

        let start = ops.start(strategy);
        if !start.ok {
            row.error = start.message.clone();
            log::warn(format!("Тест: {} — не запустилась: {}", strategy.name, start.message));
            ops.kill_winws();
            rows.push(row.clone());
            row_done(&row);
            continue;
        }
        std::thread::sleep(Duration::from_millis(1200));
        if cancelled() {
            ops.kill_winws();
            break;
        }

        // Параллельная проверка целей.
        let pairs: Vec<(usize, TestTarget)> = groups
            .iter()
            .enumerate()
            .flat_map(|(gi, g)| {
                g.targets
                    .iter()
                    .filter(|t| !t.baseline_ok)
                    .map(move |t| (gi, t.clone()))
            })
            .collect();
        let timeout = options.timeout_ms;
        let deep = options.deep_check;
        let mut results: Vec<(usize, bool, u64)> = Vec::new();
        {
            let index = std::sync::atomic::AtomicUsize::new(0);
            let results_mutex = std::sync::Mutex::new(Vec::new());
            std::thread::scope(|scope| {
                for _ in 0..6.min(pairs.len().max(1)) {
                    let index = &index;
                    let pairs = &pairs;
                    let results_mutex = &results_mutex;
                    scope.spawn(move || loop {
                        let i = index.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if i >= pairs.len() || cancelled() {
                            break;
                        }
                        let (gi, target) = &pairs[i];
                        let (ok, ms, _err) = check_target(target, timeout, deep);
                        if let Ok(mut list) = results_mutex.lock() {
                            list.push((*gi, ok, ms));
                        }
                    });
                }
            });
            if let Ok(list) = results_mutex.into_inner() {
                results = list;
            }
        }
        for (gi, ok, ms) in results {
            if let Some(group) = row.groups.get_mut(gi) {
                if ok {
                    group.passed += 1;
                }
                if ms > group.ms {
                    group.ms = ms;
                }
            }
        }

        ops.kill_winws();
        std::thread::sleep(Duration::from_millis(250));
        rows.push(row.clone());
        row_done(&row);
    }

    // Восстановление исходного состояния.
    ops.kill_winws();
    std::thread::sleep(Duration::from_millis(300));
    if options.restore_after {
        if service_was_running {
            if service::start("zapret").is_ok() {
                log::info("Тест: служба zapret запущена снова.");
            }
        } else if before.winws_running && !before.matched_strategy.is_empty() {
            if let Some(prev) = install.find_strategy(&before.matched_strategy) {
                let r = ops.start(prev);
                if !r.ok {
                    log::warn(format!("Не удалось восстановить ускорение после теста: {}", r.message));
                }
            }
        } else if before.winws_running {
            log::warn("Тест: ускорение было запущено, но стратегия не сопоставлена — после теста он остался выключен.");
        }
    }
    rows
}

// ----------------------------------------------------------------------
// Сохранение и загрузка результатов
// ----------------------------------------------------------------------

pub fn report_path() -> PathBuf {
    super::paths::data_dir().join("test-results.tsv")
}

pub fn save_tsv(
    rows: &[StrategyTestRow],
    group_names: &[String],
    options: &TesterOptions,
) -> std::io::Result<()> {
    save_tsv_to(&report_path(), rows, group_names, options)
}

pub fn save_tsv_to(
    path: &Path,
    rows: &[StrategyTestRow],
    group_names: &[String],
    options: &TesterOptions,
) -> std::io::Result<()> {
    let now = super::sys::local_now();
    let mut text = String::new();
    text.push_str("# Zapret Manager — результаты прогона стратегий\r\n");
    text.push_str(&format!(
        "# {:02}.{:02}.{:04} {:02}:{:02}:{:02}; таймаут {} мс{}\r\n",
        now.day,
        now.month,
        now.year,
        now.hour,
        now.minute,
        now.second,
        options.timeout_ms,
        if options.deep_check {
            "; глубокая проверка 16 КБ"
        } else {
            ""
        }
    ));
    text.push_str("groups");
    for g in group_names {
        text.push('\t');
        text.push_str(g);
    }
    text.push_str("\r\n");
    for row in rows {
        text.push_str(&row.strategy_name.replace(['\t', '\r', '\n'], " "));
        for g in group_names {
            text.push('\t');
            match row.groups.iter().find(|r| r.group.eq_ignore_ascii_case(g)) {
                Some(r) => text.push_str(&format!("{}/{}:{}", r.passed, r.total, r.ms)),
                None => text.push('-'),
            }
        }
        text.push('\t');
        text.push_str(&format!("{}/{}", row.passed(), row.total()));
        if !row.error.is_empty() {
            text.push('\t');
            text.push('!');
            text.push_str(&row.error.replace(['\t', '\r', '\n'], " "));
        }
        text.push_str("\r\n");
    }
    textfile::write_text_atomic(path, &text)
}

pub struct LoadedReport {
    pub groups: Vec<String>,
    pub rows: Vec<StrategyTestRow>,
    pub header: String,
}

pub fn load_tsv(path: &Path) -> Option<LoadedReport> {
    let text = textfile::read(path);
    if text.trim().is_empty() {
        return None;
    }
    let mut groups: Vec<String> = Vec::new();
    let mut rows: Vec<StrategyTestRow> = Vec::new();
    let mut header = String::new();
    let mut in_rows = false;
    for raw in text.replace("\r\n", "\n").split('\n') {
        if raw.starts_with('#') {
            if header.is_empty() && raw.contains(' ') && !raw.contains("Zapret") {
                header = raw.trim_start_matches('#').trim().to_string();
            }
            continue;
        }
        if !in_rows {
            if let Some(rest) = raw.strip_prefix("groups\t") {
                groups = rest.split('\t').map(|s| s.to_string()).collect();
                in_rows = true;
            }
            continue;
        }
        if raw.trim().is_empty() {
            continue;
        }
        let cells: Vec<&str> = raw.split('\t').collect();
        if cells.is_empty() {
            continue;
        }
        let mut row = StrategyTestRow {
            strategy_name: cells[0].to_string(),
            ..Default::default()
        };
        for (i, g) in groups.iter().enumerate() {
            let cell = cells.get(i + 1).copied().unwrap_or("-");
            let (passed, total, ms) = if cell == "-" {
                (0, 0, 0)
            } else {
                parse_cell(cell)
            };
            row.groups.push(GroupResult {
                group: g.clone(),
                passed,
                total,
                ms,
            });
        }
        if let Some(last) = cells.last() {
            if let Some(err) = last.strip_prefix('!') {
                row.error = err.to_string();
            }
        }
        rows.push(row);
    }
    if !in_rows {
        return None;
    }
    sort_rows(&mut rows);
    Some(LoadedReport {
        groups,
        rows,
        header,
    })
}

fn parse_cell(cell: &str) -> (usize, usize, u64) {
    let (score, ms) = match cell.split_once(':') {
        Some((s, m)) => (s, m.parse::<u64>().unwrap_or(0)),
        None => (cell, 0),
    };
    match score.split_once('/') {
        Some((p, t)) => (p.parse().unwrap_or(0), t.parse().unwrap_or(0), ms),
        None => (0, 0, ms),
    }
}

// ----------------------------------------------------------------------
// Официальный отчёт (utils\test results\test_results_*.txt)
// ----------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct OfficialReport {
    pub file: PathBuf,
    pub created: String,
    pub best: String,
    pub rows: Vec<OfficialTestRow>,
}

#[derive(Debug, Clone, Default)]
pub struct OfficialTestRow {
    pub name: String,
    pub ok: usize,
    pub fail: usize,
    pub unsup: usize,
    pub blocked: usize,
}

impl OfficialTestRow {
    pub fn total(&self) -> usize {
        self.ok + self.fail + self.unsup + self.blocked
    }
    pub fn percent(&self) -> usize {
        if self.total() == 0 {
            0
        } else {
            (self.ok * 100 + self.total() / 2) / self.total()
        }
    }
    pub fn details(&self) -> String {
        format!(
            "OK {} · FAIL {} · BLOCKED {} · {}%",
            self.ok,
            self.fail,
            self.blocked,
            self.percent()
        )
    }
}

pub fn reports_dir(install: &ZapretInstall) -> PathBuf {
    install.utils_dir().join("test results")
}

pub fn list_reports(install: &ZapretInstall) -> Vec<PathBuf> {
    let dir = reports_dir(install);
    let mut list = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p.file_name().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            if name.starts_with("test_results_") && name.ends_with(".txt") {
                list.push(p);
            }
        }
    }
    list.sort();
    list.reverse();
    list
}

pub fn parse_official_report(path: &Path) -> OfficialReport {
    let mut report = OfficialReport {
        file: path.to_path_buf(),
        ..Default::default()
    };
    if let Ok(meta) = std::fs::metadata(path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(secs) = modified.duration_since(std::time::UNIX_EPOCH) {
                let dt = crate::util::datetime_from_secs(secs.as_secs() as i64);
                report.created = format!(
                    "{:02}.{:02}.{:04} {:02}:{:02}",
                    dt.day, dt.month, dt.year, dt.hour, dt.minute
                );
            }
        }
    }
    let mut in_analytics = false;
    for raw in textfile::read(path).lines() {
        let line = raw.trim();
        if line.to_lowercase().starts_with("=== analytics") {
            in_analytics = true;
            continue;
        }
        if let Some(rest) = line
            .to_lowercase()
            .strip_prefix("best strategy:")
            .map(|_| &line["Best strategy:".len().min(line.len())..])
        {
            report.best = rest.trim().to_string();
            continue;
        }
        if !in_analytics {
            continue;
        }
        if let Some(row) = parse_official_row(line) {
            report.rows.push(row);
        }
    }
    report.rows.sort_by(|a, b| b.ok.cmp(&a.ok));
    report
}

fn parse_official_row(line: &str) -> Option<OfficialTestRow> {
    // name : OK: n, FAIL: n, UNSUP: n, BLOCKED: n
    let (name, rest) = line.split_once(':')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let lower = rest.to_lowercase();
    let mut row = OfficialTestRow {
        name: name.to_string(),
        ..Default::default()
    };
    if lower.contains("http ok:") {
        // name : HTTP OK: n, ERR: n, UNSUP: n, Ping OK: n, Fail: n
        let parts = split_stats(rest);
        row.ok = parts.get("http ok").copied().unwrap_or(0);
        let err = parts.get("err").copied().unwrap_or(0);
        row.unsup = parts.get("unsup").copied().unwrap_or(0);
        let ping_ok = parts.get("ping ok").copied().unwrap_or(0);
        let ping_fail = parts.get("fail").copied().unwrap_or(0);
        row.ok += ping_ok;
        row.fail = err + ping_fail;
        Some(row)
    } else if lower.contains("ok:") {
        let parts = split_stats(rest);
        row.ok = parts.get("ok").copied().unwrap_or(0);
        row.fail = parts.get("fail").copied().unwrap_or(0);
        row.unsup = parts.get("unsup").copied().unwrap_or(0);
        row.blocked = parts.get("blocked").copied().unwrap_or(0);
        Some(row)
    } else {
        None
    }
}

fn split_stats(rest: &str) -> std::collections::HashMap<String, usize> {
    let mut map = std::collections::HashMap::new();
    for part in rest.split(',') {
        if let Some((key, value)) = part.split_once(':') {
            let key = key.trim().to_lowercase();
            let value = value.trim().parse::<usize>().unwrap_or(0);
            map.insert(key, value);
        }
    }
    map
}

/// Запустить официальный скрипт теста в отдельном окне PowerShell.
pub fn run_official_test(install: &ZapretInstall) -> Result<(), String> {
    let script = install.utils_dir().join("test zapret.ps1");
    if !script.is_file() {
        return Err("Не найден файл utils\\test zapret.ps1".to_string());
    }
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-ExecutionPolicy",
        "Ускорение",
        "-File",
        &script.to_string_lossy(),
    ])
    .current_dir(&install.root);
    cmd.spawn().map(|_| ()).map_err(|e| format!("Не удалось запустить тест: {}", e))
}

/// Стратегия по имени из отчёта (с .bat или без).
pub fn find_strategy<'a>(install: &'a ZapretInstall, name: &str) -> Option<&'a StrategyDef> {
    let trimmed = name.trim().trim_end_matches(".bat");
    install.find_strategy(trimmed)
}

/// Путь к файлу стратегии для показа в проводнике.
pub fn strategy_file(install: &ZapretInstall, name: &str) -> Option<PathBuf> {
    find_strategy(install, name).map(|s| s.file.clone())
}

/// Командная строка стратегии (для подсказок в интерфейсе).
pub fn strategy_command(install: &ZapretInstall, strategy: &StrategyDef) -> String {
    let gf = GameFilterState::load(&install.root);
    match bat::get_argv(install, strategy, &gf) {
        Ok(argv) => format!(
            "\"{}\" {}",
            install.winws_exe().to_string_lossy(),
            bat::build_command_line(&argv)
        ),
        Err(e) => format!("Ошибка разбора: {}", e),
    }
}

#[allow(dead_code)]
fn unused(_: &ZapretOps, _: &process::ProcResult) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_health_threshold_is_sixty_percent() {
        let group = |passed, total| GroupResult {
            group: "x".to_string(),
            passed,
            total,
            ms: 0,
        };
        assert!(group(3, 5).healthy());
        assert!(group(2, 5).healthy() == false);
        assert!(group(0, 0).healthy() == false);
        let row = StrategyTestRow {
            groups: vec![group(3, 5), group(3, 4), group(0, 1)],
            ..Default::default()
        };
        assert_eq!(row.healthy_groups(), 2);
        assert_eq!(row.score(), (2, 6));
    }

    #[test]
    fn sort_rows_places_best_first_and_keeps_ties() {
        let group = |passed: usize, total: usize| GroupResult {
            group: "g".to_string(),
            passed,
            total,
            ms: 0,
        };
        let make = |name: &str, groups: Vec<GroupResult>| StrategyTestRow {
            strategy_name: name.to_string(),
            groups,
            ..Default::default()
        };
        let mut rows = vec![
            make("bad", vec![group(0, 4)]),
            make("good-b", vec![group(4, 4), group(3, 4)]),
            make("good-a", vec![group(4, 4), group(3, 4)]),
            make("middle", vec![group(2, 4)]),
        ];
        sort_rows(&mut rows);
        assert_eq!(rows[0].strategy_name, "good-a");
        assert_eq!(rows[1].strategy_name, "good-b");
        assert_eq!(rows[2].strategy_name, "middle");
        assert_eq!(rows[3].strategy_name, "bad");
        assert_eq!(rows[0].score(), rows[1].score());
    }
}
