# Безопасность

## Ложное срабатывание `Trojan:Win32/Bearfoos.A!ml`

Defender иногда помечает `ZapretManager.exe` как `Trojan:Win32/Bearfoos.A!ml`.
Это **ложное срабатывание ML-эвристики**: утилита управляет сетью, службами и
драйвером WinDivert, поэтому по поведению похожа на dropper, хотя это её
штатная работа.

### Проверьте файл

В каждом релизе рядом с exe лежит `checksums.txt` (SHA-256 считается после
подписи, если она есть):

```powershell
Get-FileHash "C:\путь\к\ZapretManager.exe" -Algorithm SHA256
Get-Content "C:\путь\к\checksums.txt"
```

Хэши должны совпасть. Если нет — это не наш файл: удалите его и скачайте
релиз заново с https://github.com/RoBiN19666/ZapretManager-Desktop/releases/latest

### Что делать

1. **Безопасность Windows → Защита от вирусов и угроз → Журнал защиты**.
2. Найти обнаружение `Trojan:Win32/Bearfoos.A!ml`.
3. **Действия → Разрешить на устройстве**.

Не отключайте Defender целиком. Исключение — только для папки с программой
(например, `C:\ZapretManager`), не для `Downloads`.

## Что делает проект против ложных срабатываний

- В exe встроен манифест (`asInvoker`, версии Windows, DPI, long paths) и
  стабильные метаданные версии; упаковщика (UPX) нет.
- exe собирается в GitHub Actions из тега, `tools/verify-exe.ps1` в CI
  проверяет манифест и версию.
- Самообновление сверяет SHA-256, размер и Authenticode-подпись файла.

## Подпись кода

Релизы подписываются Authenticode в CI, когда в репозитории заданы секреты
SignPath (`SIGNPATH_API_TOKEN` + переменные `SIGNPATH_*`) или свой сертификат
(`SIGNING_PFX_BASE64` + `SIGNING_PFX_PASSWORD`). Пока их нет, релизы выходят
неподписанными, и workflow пишет об этом предупреждение.

## Ложное срабатывание в Microsoft

https://www.microsoft.com/en-us/wdsi/filesubmission

Укажите обнаружение `Trojan:Win32/Bearfoos.A!ml`, приложите `ZapretManager.exe`
из релиза и коротко опишите, что это open-source менеджер для `zapret`.
Заявку имеет смысл повторять после заметных релизов.

## Уязвимости

Не публикуйте проблему сразу: создайте private security advisory в GitHub или
напишите автору через https://github.com/RoBiN19666.
