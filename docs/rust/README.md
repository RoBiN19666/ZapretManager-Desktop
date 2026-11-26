# Rust-переписка: заметки о миграции

Ветка `rust-rewrite` — полная переписка C# WinForms-приложения на Rust.
C#-исходники остаются в ветке `main`; здесь их нет.

## Что перенесено 1:1

- `BatParser` — разбор `.bat`-стратегий: `set`-переменные, `%~dp0`, `%VAR%`, продолжение строк `^`,
  кавычки и сборка командной строки (`engine/bat.rs`, есть тесты).
- `GameFilterState` — `utils/game_filter.enabled`, токены `%GameFilterTCP%`/`%GameFilterUDP%`.
- `ZapretInstall` — поиск папки (настройки → служба → exe → подпапки → `C:\zapret`),
  версия из `LOCAL_VERSION`, `version.txt`, имени папки; естественная сортировка стратегий.
- `ZapretOps` — запуск `winws.exe` (raw-командная строка без переэкранирования), проверка
  «упал сразу/через пару секунд», `kill_winws` только для своей папки, служба, IPSet, фейки,
  hosts, кэш Discord, резервные копии.
- `ListChanges`/`ListAudit` — блок «мои добавления», `.baseline`, `.changes`, `.news`,
  применение после обновления, возврат удалённых, перенос в `*-user.txt`, вставка и чистка.
- `SiteDoctor` — этапы DNS/TCP/TLS/HTTP с теми же таймаутами, вердиктами и текстами;
  связанные домены (корень, `www/m/api/static/cdn/i`, хосты из HTML), запись в список и hosts.
- `SiteGuard` — журнал `Microsoft-Windows-DNS-Client/Operational`, фильтр браузеров,
  шум-лист, «не чаще раза в час», максимум 5 проверок за цикл, авто-добавление.
- `AppProbe` — процессы с окнами, та же программа и дочерние процессы, `netstat` для IP,
  наблюдение по шагам.
- `StrategyTester` — цели из `targets.txt`, базовая доступность без обхода, глубокий тест 16 КБ,
  страницы-заглушки, TSV-отчёт, разбор официальных `test_results_*.txt`.
- `Diagnostics` — порядок проверок, тексты, исправления (`windivert`, `conflicts`, `tcp`,
  `discord`, `hosts`).
- `Update`/`SelfUpdate` — GitHub API, резервная копия, откат, сохранение пользовательских
  файлов, `--apply-update` без `cmd.exe`.
- `TgProxy` — поиск установки, `config.json`, `tg://`-ссылка, запуск/остановка, установка релиза.
- `Autostart` — задача `ZapretManager` с `ONLOGON`/`HIGHEST`, но через XML (нет проблем с кавычками).
- Конфиг `data\manager.ini` — тот же формат и те же ключи, совместим с C#-версией.

## Сознательные отличия

| Что | C# | Rust |
| --- | --- | --- |
| Служба | `sc.exe create/start/delete` | SCM API (`CreateServiceW`, `ControlService`, `DeleteService`) |
| HTTP | `HttpWebRequest`, TLS 1.2/1.3 | WinHTTP + системный TLS, `Accept-Encoding: identity` |
| DNS-журнал | `EventLogReader` | `wevtutil qe ... /f:xml` + разбор XML |
| Процессы | WMI (`Win32_Process`) | Toolhelp + `NtQueryInformationProcess(60)` |
| Автозапуск | `schtasks /Create /TR "..."` | XML-задача + `schtasks /Create /XML` |
| Поток UI | WinForms + `BeginInvoke` | egui immediate mode + канал сообщений из фоновых потоков |
| Тихий TG-прокси | портативный Python + pip | не перенесён (страница сообщает об этом) |

## Сборка и подводные камни

- GNU-тулчейн (`x86_64-pc-windows-gnu`) не линкует проект, если в пути есть кириллица:
  задайте `CARGO_TARGET_DIR` в ASCII-папку (это делает `build.ps1`).
- Иконки и шрифты: PNG/ICO встроены в exe через `include_bytes!`; Segoe UI и Consolas
  читаются из `C:\Windows\Fonts` во время запуска (если файла нет — остаются шрифты egui).
- Скриншоты для документации: `--shot=<png> --shot-page=<key>` (egui screenshot API).

## Что осталось на будущее

- Тихий режим TG WS Proxy (TgSource) — скачивание исходников и портативного Python.
- Настройка прокси и пользовательские темы сверх трёх пресетов.
- Автотесты UI (сейчас проверяются скриншотами и headless-режимами `--diag`, `--check-site`).
