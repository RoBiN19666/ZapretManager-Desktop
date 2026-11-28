# Zapret Manager

![Zapret Manager](assets/logo-wide.png)

Графическая оболочка для **[zapret-discord-youtube](https://github.com/Flowseal/zapret-discord-youtube)** —
сборки [zapret](https://github.com/bol-van/zapret) (автор **bol-van**) для ускорения соединения
Discord и YouTube, — и для **[TG WS Proxy](https://github.com/Flowseal/tg-ws-proxy)** (прокси
Telegram через WebSocket).

Приложение написано на **Rust**: один нативный `ZapretManager.exe` без внешних зависимостей
(GUI на `egui`/`eframe`, системный WinHTTP, Win32 API). Сам менеджер трафик не изменяет:
он находит установку zapret, запускает и останавливает её стратегии, ведёт списки доменов,
проверяет доступность сайтов, следит за обновлениями и держит прокси Telegram.

История версий — в [CHANGELOG.md](CHANGELOG.md). Руководство пользователя — в
[docs/guide.md](docs/guide.md).

![Обзор](docs/rust/screenshots/overview.png)

## Что умеет

| Раздел | Возможности |
| --- | --- |
| **Обзор** | Состояние защиты, стратегия/PID/uptime, запуск–остановка–перезапуск, быстрые переключатели игрового фильтра, IP-списка и авто-проверки, **память winws/TG WS Proxy**, живой журнал |
| **Стратегии** | Все `general*.bat` с описаниями и метками BASE/ALT/EXP, **поиск и избранное ★**, предпросмотр и копирование полной команды `winws.exe`, запуск, установка службой Windows |
| **Тест стратегий** | Прогон всех стратегий по целям из `utils\targets.txt`, таблица «да/нет/1 из N», **сортировка по числу прошедших целей, затем по сервисам (60%)**, ★ у лучшей, **долгий автоподбор desync-параметров** (включая `hostfakesplit`, `fake-tls-mod`, `badseq`), сохранение `general (AUTO).bat`, **применение с проверкой и откатом**, отчёт в `data\test-results.tsv`, разбор официального отчёта `utils\test zapret.ps1` |
| **Проверка сайта** | Пошаговая диагностика DNS → TCP → TLS → HTTP, вердикт (ok / dns / tls / http / down), подбор связанных доменов, добавление в свой список или `hosts`, **сторож с журналом перехваченных доменов**, наблюдение за приложением по журналу DNS |
| **TG-прокси** | Поиск установленного TG WS Proxy, состояние и порт, запуск/остановка/перезапуск, ссылка `tg://proxy`, установка обновлений с GitHub, **тихий режим из исходников без иконки в трее**, автозапуск |
| **Настройки** | **Папка zapret (выбор/установка с нуля)**, игровой фильтр и порты, режимы IPSet, обновление IP-листа, замена активных фейк-пакетов, тема и масштаб, TCP timestamps, очистка кэша Discord, автозапуск через планировщик, **автолечение ускорения** |
| **Списки** | Редактор 8 файлов списков, подсветка дублей и `*.`, массовая вставка и «причёсывание», **пометка своих изменений** в официальных списках (`baseline`/`changes`/`news`) с переносом при обновлении zapret |
| **Установленные версии** | Все компоненты в одном месте: менеджер, zapret, TG WS Proxy — текущая и доступная версия, **«Проверить всё»** и **«Обновить всё»**, установка любой версии zapret, резервные копии |
| **Диагностика** | около 30 проверок: BFE, службы-конфликты (Killer, Intel, Check Point, SmartByte, VPN, сторонние ускорители), прокси, TCP timestamps, DoH, hosts, целостность списков, **аудит и докачивание fake-файлов (blobs)**, зависший WinDivert, TG WS Proxy — с кнопками исправления |
| **Инструменты** | Служба Windows, файл hosts (официальный список), официальный тест стратегий, кэш Discord, папки, **резервная копия настроек и списков `.zmb`** |
| **О программе** | Крупный логотип, версия, компоненты, лицензия, ссылки и поддержка проекта |

Скриншоты: [Стратегии](docs/rust/screenshots/strategies.png) ·
[Тест](docs/rust/screenshots/test.png) · [Списки](docs/rust/screenshots/lists.png) ·
[Настройки](docs/rust/screenshots/settings.png) · [TG-прокси](docs/rust/screenshots/tgproxy.png) ·
[Диагностика](docs/rust/screenshots/diagnostics.png) · [О программе](docs/rust/screenshots/about.png)

## Какое ядро zapret поддерживается

Менеджер работает с **zapret 1.x**: `bin\winws.exe` и стратегии `general*.bat` с
параметрами `--dpi-desync…`.

Flowseal выпускает **zapret 2.x** — это фактически другой продукт: `bin\winws2.exe`,
каталог `lua\` и правила `--lua-desync`. Такую установку менеджер **распознаёт**, но:

- показывает предупреждение «другое ядро» на всех страницах;
- не запускает её стратегии, не ставит службу, не прогоняет тест и автоподбор;
- **не предлагает обновление** на 2.x и на тестовые сборки (prerelease) — ни в
  «Проверить обновления», ни в списке версий, ни при установке с нуля.

Управлять такой установкой нужно её собственным `service.bat`. Если нужно
вернуться на поддерживаемую ветку, поставьте любую версию 1.x на странице
«Обновление» → «Версия zapret».

## Сборка

Нужен **Rust 1.88+** для Windows (x86_64). Проще всего установить через [rustup](https://rustup.rs).

```powershell
# из корня репозитория
cargo build --release
# готовый файл: target\release\ZapretManager.exe
```

Или скриптом (сам выставит ASCII-путь сборки и соберёт `dist\` с контрольной суммой):

```powershell
.\build.ps1
```

**Про GNU-тулчейн.** Если Rust установлен с `x86_64-pc-windows-gnu`, линковщик MinGW не умеет
работать с путями, где есть кириллица или пробелы. Поэтому `build.ps1` и CI собирают с
`CARGO_TARGET_DIR` во временной ASCII-папке; при ручной сборке из такой папки задайте её сами:

```powershell
$env:CARGO_TARGET_DIR = "$env:TEMP\zapret-build"
cargo build --release
```

С MSVC-тулчейном (`x86_64-pc-windows-msvc`) это не требуется.

## Запуск

```
ZapretManager.exe                 — обычный запуск (UAC запросится, если нужно)
ZapretManager.exe --tray          — свернуться в трей
ZapretManager.exe --autostart     — тихий старт (задача планировщика)
ZapretManager.exe --root=<папка>  — указать папку zapret вручную
ZapretManager.exe --diag          — прогнать диагностику в журнал и выйти
ZapretManager.exe --selftest      — внутренняя самопроверка (11 проверок) и выход с кодом 0/1
ZapretManager.exe --check-site=<домен> — проверить сайт из командной строки
ZapretManager.exe --app-watch=<PID>    — наблюдение за приложением и добавление найденного
ZapretManager.exe --guard-once    — один проход фонового сторожа
ZapretManager.exe --blobs-repair  — проверить и докачать fake-файлы
ZapretManager.exe --cleanup       — убрать старые резервные копии и временные файлы
ZapretManager.exe --restore-uac   — вернуть стандартные значения UAC (5/1)
ZapretManager.exe --tg-prepare    — скачать исходники TG WS Proxy и портативный Python
ZapretManager.exe --tg-start      — запустить TG WS Proxy (с учётом выбранного режима)
ZapretManager.exe --tg-stop       — остановить TG WS Proxy
ZapretManager.exe --check-self    — проверить обновление менеджера
ZapretManager.exe --self-update   — обновиться без интерфейса
ZapretManager.exe --apply-update  — служебный режим замены exe (вызывается автоматически)
ZapretManager.exe --limited       — работать без прав администратора
ZapretManager.exe --shot=<файл> --shot-page=<страница> — снимок окна для документации
```

Журнал и настройки — в `data\` рядом с exe (если папка недоступна для записи —
`%APPDATA%\ZapretManager`). Формат `manager.ini` — `key=value`, его можно править вручную.

## Как это устроено

```
src/
  main.rs            — точка входа, CLI-режимы, UAC, один экземпляр
  lib.rs             — библиотека (движок + интерфейс), версия приложения
  build.rs           — встраивание иконки, манифеста и версии в exe (windres/rc.exe)
  util.rs            — версии, натуральная сортировка, SHA-256, время, валидация
  engine/            — вся логика без UI
    install.rs       — поиск установки zapret, ядро (1.x/2.x), разбор стратегий и версии
    bat.rs           — превращение .bat-стратегии в аргументы winws.exe и обратно
    ops.rs           — запуск/остановка winws, служба, IPSet, фейки, hosts, кэш Discord
    service.rs       — Service Control Manager (без sc.exe)
    process.rs, sys.rs — Win32: процессы, реестр, UAC, окна, значок в трее
    http.rs          — HTTP-клиент на WinHTTP (системный TLS, редиректы, прогресс)
    netcheck.rs      — DNS/TCP/TLS/HTTP/ICMP-проверки
    doctor.rs        — доктор сайтов, связанные адреса, запись в списки/hosts
    guard.rs         — фоновый сторож по журналу DNS (wevtutil)
    appprobe.rs      — проверка приложения
    lists.rs         — пометка изменений официальных списков, чистка, вставка
    tester.rs        — тест стратегий и разбор официальных отчётов
    autotune.rs      — автоподбор desync-параметров
    diagnostics.rs   — проверки и исправления
    blobs.rs         — аудит и докачивание fake-файлов
    update.rs        — GitHub Releases, установка zapret, самообновление
    tgproxy.rs       — TG WS Proxy (exe с треем и тихий режим)
    tgsource.rs      — тихий режим: исходники Flowseal + портативный Python
    prefs.rs, config.rs, paths.rs, textfile.rs, log.rs, backup.rs, uninstall.rs,
    autostart.rs, gamefilter.rs, winverify.rs, selftest.rs
  ui/                — egui/eframe
    app.rs           — окно, сайдбар, статусбар, фоновые задачи, уведомления
    theme.rs         — тёмная/светлая палитра, системные шрифты Segoe UI/Consolas
    widgets.rs       — карточки, кнопки, переключатели, прогресс
    tray.rs          — Shell_NotifyIcon и собственное меню в отдельном потоке
    pages/           — 11 страниц + мастер первой настройки
```

Внешние крейты: `eframe`/`egui` (GUI), `windows-sys` (Win32), `native-tls` (TLS-проверка с SNI),
`zip` (распаковка обновлений), `serde_json` (GitHub API), `png` (иконки), `rfd` (диалоги файлов).
Подробнее о внутреннем устройстве — [docs/rust/README.md](docs/rust/README.md).

**Решения, которые стоит знать:**

- Служба создаётся через SCM API, а не `sc.exe`; автозапуск — через XML-задачу планировщика.
- HTTP — WinHTTP с системным TLS 1.2/1.3; тело ответа ограничено, `https → http` запрещён.
- Журнал DNS читается через `wevtutil` (XML), а не через API журналов.
- Список процессов и командные строки — Toolhelp + `NtQueryInformationProcess`, без WMI.
- **Тихий режим TG WS Proxy**: страница «TG-прокси» скачивает официальные исходники
  Flowseal и портативный Python, запускает прокси через `pythonw.exe` — без окна и без
  иконки в трее. Системный Python не нужен.
- Самообновление менеджера рассчитано на публичные GitHub Releases: обязательный
  `checksums.txt`, проверка подписи и размера, безопасная замена exe.
- Иконки разделов — из системного шрифта Segoe MDL2 Assets (Windows 10/11), с запасными символами.
- Окно запоминает размер и состояние «развёрнуто»; заголовок окрашивается под тему.
- Для приватного репозитория самообновление можно включить переменной `ZAPRET_GITHUB_TOKEN`
  (токен с доступом к релизам): тогда проверка и скачивание идут через GitHub API.

## Проверки

```powershell
cargo test      # 51 тест + 1 сетевой (--ignored): .bat-парсер, списки, версии, ядра, SHA-256, время
cargo test -- --ignored   # сетевой тест: реальный GitHub API пропускает zapret 2.x
cargo clippy    # по желанию
```

Дополнительно: `ZapretManager.exe --selftest` (11 проверок без сети и изменений системы)
и `ZapretManager.exe --diag` (около 30 проверок окружения в журнал).

## Антивирус и ложные срабатывания

Defender иногда помечает `ZapretManager.exe` как `Trojan:Win32/Bearfoos.A!ml` —
это ложное срабатывание ML-эвристики (утилита управляет сетью, службами и
WinDivert).

Сверьте SHA-256 с `checksums.txt` из релиза
(`Get-FileHash .\ZapretManager.exe -Algorithm SHA256`). Если хэш совпал —
восстановите файл из карантина и отправьте ложное срабатывание в Microsoft.
Подробнее: [SECURITY.md](SECURITY.md).

## Подпись кода

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by
[SignPath Foundation](https://signpath.org/).

Релизы собираются в GitHub Actions и подписываются Authenticode, когда в
репозитории заданы секреты подписи; до этого выходят неподписанными. Роли:
коммиты, ревью и утверждение подписи — владелец репозитория. Телеметрии нет,
сеть используется только для обновлений и проверок по действию пользователя.
Подробнее: [SECURITY.md](SECURITY.md).

## Благодарности

Проект существует только потому, что есть люди, которые сделали и продолжают делать
сам обход блокировок:

- **[bol-van/zapret](https://github.com/bol-van/zapret)** (bol-van) — ядро обхода DPI,
  на котором всё держится. Спасибо за `winws` и подробную документацию.
- **[Flowseal/zapret-discord-youtube](https://github.com/Flowseal/zapret-discord-youtube)**
  (Flowseal и контрибьюторы) — сборка zapret под Discord/YouTube, стратегии `general*.bat`,
  `service.bat`, списки и фейк-пакеты, `utils/targets.txt` и официальный тест стратегий.
- **[Flowseal/tg-ws-proxy](https://github.com/Flowseal/tg-ws-proxy)** (Flowseal) —
  MTProto-прокси Telegram через WebSocket, включая тихий режим из исходников.
- **[basil00/WinDivert](https://github.com/basil00/WinDivert)** — драйвер перехвата пакетов,
  без которого zapret не работает.
- Сообществу проекта zapret — за обсуждения стратегий, отчёты и тесты на живых провайдерах.

Отдельная благодарность авторам библиотек, на которых собран менеджер:
[egui/eframe](https://github.com/emilk/egui) (emilk и участники), `windows-sys`,
[native-tls](https://github.com/sfackler/rust-native-tls), [zip-rs](https://github.com/zip-rs/zip2),
[serde_json](https://github.com/serde-rs/json), [rfd](https://github.com/PolyMeilex/rfd).

Zapret Manager — неофициальная оболочка: она не связана с bol-van, Flowseal и Microsoft.
Все права на zapret, WinDivert и TG WS Proxy принадлежат их авторам и распространяются
по их лицензиям.

## Лицензия

MIT — см. [LICENSE](LICENSE). Copyright (c) 2026 RoBiN19666.

Проект не содержит код zapret, zapret-discord-youtube, TG WS Proxy и WinDivert: они скачиваются
с официальных источников и распространяются по своим лицензиям — перечень и ссылки
в [NOTICE.md](NOTICE.md).

---

Проект сделан с помощью инструмента **[DeepSeek Harness](https://deepseek.com)** — агентной среды
разработки: код, документация и проверки писались и ревьюились в ней.
