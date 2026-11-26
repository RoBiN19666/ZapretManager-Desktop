# Безопасность и ложные срабатывания антивирусов

## `Trojan:Win32/Bearfoos.A!ml` в Microsoft Defender

Defender иногда помечает `ZapretManager.exe` как
`Trojan:Win32/Bearfoos.A!ml`. Это **ложное срабатывание ML-эвристики**, а не
признак заражения сборки.

Причина: приложение управляет сетью и службами Windows — запускает
`winws.exe`, работает с драйвером WinDivert, создаёт службу, читает журнал
DNS, обновляет списки и самообновляется. Для эвристики это похоже на
dropper/persistence-поведение, хотя это штатная работа менеджера ускорения.

### Сначала проверьте файл

Для каждого релиза рядом с `ZapretManager.exe` публикуется `checksums.txt`
с SHA-256 — хэш считается уже **после** подписи, поэтому он совпадает с тем
файлом, который вы скачали:

```powershell
Get-FileHash "C:\путь\к\ZapretManager.exe" -Algorithm SHA256
Get-Content "C:\путь\к\checksums.txt"
```

Хэши должны совпасть. Если нет — это не наш файл: удалите его и скачайте
релиз заново со страницы
https://github.com/RoBiN19666/ZapretManager-Desktop/releases/latest

Дополнительно можно проверить файл на VirusTotal (детекты 2–5 из 70 на
неподписанную утилиту такого класса — обычное дело) и посмотреть, какие
именно движки сработали.

### Если хэш совпадает

1. Откройте **Безопасность Windows → Защита от вирусов и угроз → Журнал
   защиты**.
2. Найдите обнаружение `Trojan:Win32/Bearfoos.A!ml`.
3. Нажмите **Действия → Разрешить на устройстве**.

Не отключайте Defender полностью. Если добавляете исключение — только для
конкретной папки с программой, например `C:\ZapretManager`, а не для
`Downloads` или `Telegram Desktop`.

Перенос из папки загрузок и снятие метки «скачано из интернета» могут помочь
SmartScreen, но не гарантируют исчезновение ML-детекта:

```powershell
Move-Item "C:\Users\<user>\Downloads\Telegram Desktop\ZapretManager.exe" "C:\ZapretManager\"
Unblock-File "C:\ZapretManager\ZapretManager.exe"
```

## Что уже сделано против ложных срабатываний

- **Манифест приложения** (`app.manifest`) встроен в exe: `asInvoker`,
  поддерживаемые версии Windows, DPI-awareness, long paths. Без манифеста
  Windows включает «installer detection» и считает неподписанный exe
  установщиком — это лишний повод для UAC и эвристик.
- **Стабильные метаданные**: `CompanyName`, `ProductName`, `FileDescription`,
  `OriginalFilename` и версия не меняются от релиза к релизу и совпадают с
  тегом.
- **Нет упаковщика**: exe не сжат UPX и не обфусцирован.
- **Сборка из публичного репозитория** в GitHub Actions
  (`.github/workflows/release.yml`): релизный exe собирается на чистом
  раннере из тега, а не на локальной машине. Это видимая provenance.
- **Проверка перед публикацией**: `tools/verify-exe.ps1` в CI падает, если в
  exe нет манифеста или ресурса версии.
- **Контрольные суммы**: `checksums.txt` в каждом релизе; самообновление
  сверяет SHA-256, размер ассета и Authenticode-подпись скачанного файла и
  отказывается ставить файл с недействительной подписью.

## Подпись кода (Authenticode)

Единственный способ убрать детект навсегда — подписать релизы доверенным
сертификатом. Для open source есть бесплатная программа
[SignPath Foundation](https://signpath.org/): сертификат привязан к
репозиторию, закрытый ключ хранится в HSM, подпись встраивается в CI.

Workflow уже готов и подписывает автоматически, как только в
**Settings → Secrets and variables → Actions** появятся:

| Тип | Имя | Значение |
| --- | --- | --- |
| Secret | `SIGNPATH_API_TOKEN` | токен SignPath |
| Variable | `SIGNPATH_ORGANIZATION_ID` | id организации |
| Variable | `SIGNPATH_PROJECT_SLUG` | слаг проекта |
| Variable | `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG` | конфигурация артефакта |
| Variable | `SIGNPATH_SIGNING_POLICY` | `release-signing` |

Пока их нет, workflow собирает неподписанный exe и предупреждает об этом
в логе.

## Отправьте ложное срабатывание в Microsoft

https://www.microsoft.com/en-us/wdsi/filesubmission

Укажите:

- обнаружение: `Trojan:Win32/Bearfoos.A!ml`;
- файл: `ZapretManager.exe` из официального релиза (лучше — подписанный);
- комментарий: неподписанный open-source менеджер для `zapret`,
  управляет WinDivert/winws, ложное ML-срабатывание.

Заявку имеет смысл повторять после каждого заметного релиза: Microsoft
пересчитывает репутацию по конкретным файлам и сертификату.

## Реальная уязвимость

Если вы нашли проблему безопасности в коде — не публикуйте её сразу.
Создайте private security advisory в GitHub или свяжитесь с автором через
профиль https://github.com/RoBiN19666.
