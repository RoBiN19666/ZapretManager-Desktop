# Спецификация: проверка сайта, сторож, проверка приложения

Источник: `src/SiteDoctor.cs`, `src/SiteGuard.cs`, `src/AppProbe.cs`, `src/PageSites.cs` (C#-версия).

## SiteDoctor (движок проверки)

### Нормализация ввода
- Обрезать пробелы и символы `" ' < >`.
- Нет `://` → добавить `https://`.
- Разобрать URL; хост в нижний регистр, IDN → punycode; URL для открытия в браузере — только `scheme://host[:port]/`.
- Пустой хост → вердикт `down`, текст «Не удалось разобрать адрес…».

### Этапы проверки (последовательно)
1. DNS (таймаут 5000 мс), IPv4 сначала, затем IPv6.
2. TCP :443 (5000 мс).
3. TLS с SNI = хост (7000 мс, сертификат не проверяется, TLS 1.2/1.3).
4. HTTP GET `https://host/` (12000 мс, UA Chrome, максимум 256 КиБ, заголовок `<title>`).

Прогресс-строки: «Определяем адрес …», «Проверяем TCP-соединение…», «Проверяем TLS-рукопожатие (SNI «…»)…», «Запрашиваем страницу…».
DNS-ошибка прерывает цепочку; TCP-ошибка — тоже; TLS и HTTP только после успеха предыдущего.

### Классификация
- `dns` — DNS не отвечает.
- `dpi` — TCP или TLS оборвались с reset/таймаутом/EOF/forcibly.
- `tls` — TCP прошёл, TLS ошибка иного рода.
- `http` — TLS прошёл, HTTP вернул ошибку.
- `down` — отказ в соединении/прочая сетевая ошибка.

Тексты вердиктов — как в C#-версии (см. PageSites).

### Связанные адреса
- Сам хост, корневой домен (последние 2 метки; для co.uk/org.uk/com.br/com.au/co.jp/co.kr/com.tr/co.in/com.mx — 3), поддомены `www, m, api, static, cdn, i`, хосты из HTML (абсолютные URL и домены в кавычках), лимит 40, фильтр мусора (`w3.org`, `schema.org`, `fonts.`, `.local`, `.lan`, IP).
- Проверка: DNS 2000 мс → TCP 2500/1500 мс → TLS 2500 мс; проблемные первыми, живые — после.

### Действия
- «В мой список» — дописать в `lists\list-general-user.txt` (нормализация, дедупликация, атомарная запись, CRLF, UTF-8 без BOM).
- «Прописать в hosts» — определить первый IPv4, удалить прошлый блок `# >>> Zapret Manager: адреса сайтов`, резервная копия `hosts.zapret-backup-yyyyMMdd-HHmmss`, новый блок с датой, `ipconfig /flushdns`.
- «Проверить адреса» — TCP 2500 + TLS 4000 по каждой строке; проблемные помечаются комментарием `# TCP: …`/`# TLS: …`.
- «Скопировать» — в буфер обмена.

## SiteGuard (фоновый сторож)
- Журнал `Microsoft-Windows-DNS-Client/Operational`, XPath `*[System[(EventID=3020 or EventID=3022 or EventID=3021)]]`, новые события первыми, максимум 200 записей за цикл.
- Домен извлекается из текста события; PID клиента — из XML `<Data Name="ClientPID">`.
- Браузеры: chrome, msedge, firefox, opera, brave, vivaldi, yandex, browser, thorium, librewolf.
- Шум-лист служебных доменов (Microsoft, Google, Cloudflare, Apple, Mozilla, GitHub, Steam, Epic, Battle.net, Riot, антивирусы и т.п.).
- Проверка каждого домена: TCP 2500 мс, при успехе TLS 4000 мс; reset/таймаут/EOF → «похоже на блокировку».
- Пропуск доменов, уже покрытых списками (`*list*.txt`, исключая `exclude`, с учётом поддоменов).
- Не чаще 1 раза в час на домен, максимум 5 проверок за цикл.
- Авто-добавление (`SiteGuardAutoAdd`) → `list-general-user.txt`; иначе уведомление.
- Включение журнала: `wevtutil sl "Microsoft-Windows-DNS-Client/Operational" /e:true`.
- Опрос каждые 6 с в UI-потоке через таймер; работа в фоне.

## AppProbe (проверка приложения)
- Список процессов с непустым заголовком окна, кроме системных (explorer, dwm, taskhostw, …), сортировка по имени.
- Та же программа = все процессы с тем же именем образа и тем же путём.
- Дочерние процессы — через `Win32_Process` (WMI) транзитивно; в Rust: Toolhelp по ParentProcessID.
- Удалённые IP — `netstat -ano -p TCP`, фильтр локальных/приватных адресов.
- Наблюдение: N секунд, шаг 2 с, DNS-журнал по разрешённым PID, объединение с IP, прогресс.
- «Заблокированные» — через SiteDoctor.LooksBlocked (TCP 2500 → TLS 4000, reset/таймаут/EOF).
- CLI `--app-watch=<PID>`: 20 секунд, затем проверка и безусловное добавление найденного в список.

## Конфиг и файлы
- `SiteGuard` (по умолчанию 0), `SiteGuardAutoAdd` (0), `Root`.
- `data\manager.ini`, `data\manager.log` (ротация 512 КиБ).
- `lists\list-general-user.txt`, `lists\*list*.txt`.
- `%SystemRoot%\System32\drivers\etc\hosts`.
