# Безопасность и ложные срабатывания антивирусов

## `Trojan:Win32/Bearfoos.A!ml` в Microsoft Defender

Defender иногда помечает `ZapretManager.exe` как
`Trojan:Win32/Bearfoos.A!ml`. Это **ложное срабатывание ML-эвристики**, а не
признак заражения сборки.

Причина: приложение не подписано сертификатом Authenticode, запускается с
правами администратора, управляет службой Windows, сетевыми процессами
`winws.exe` и драйвером WinDivert. Такое поведение похоже на подозрительное
для эвристики, хотя это штатная работа менеджера ускорения.

### Сначала проверьте файл

Официальный релиз **2.6.2**:

```text
SHA-256: 8A0D2650AF3BC85EF95BADBA85602149CE1783FA3AC80E19589BAE1242903617
Размер:  5 748 224 байта
```

Проверка в PowerShell:

```powershell
Get-FileHash "C:\путь\к\ZapretManager.exe" -Algorithm SHA256
```

Если хэш не совпадает — это не наш файл. Не добавляйте его в исключения,
удалите и скачайте релиз заново:

https://github.com/RoBiN19666/ZapretManager-Desktop/releases/latest

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

### Отправьте ложное срабатывание в Microsoft

https://www.microsoft.com/en-us/wdsi/filesubmission

Укажите:

- обнаружение: `Trojan:Win32/Bearfoos.A!ml`;
- файл: `ZapretManager.exe` из официального релиза;
- комментарий: неподписанный open-source менеджер для `zapret`,
  управляет WinDivert/winws, ложное ML-срабатывание.

### Надёжное решение

Единственный способ убрать детект навсегда — **подпись кода Authenticode**
доверенным сертификатом (OV/EV) и отправка подписанных релизов в Microsoft.
Сейчас релизы не подписаны, поэтому гарантии нет.

Если у вас есть сертификат `.pfx`, подпись можно добавить в релизный процесс
через `signtool` и GitHub Actions.

## Реальная уязвимость

Если вы нашли проблему безопасности в коде — не публикуйте её сразу.
Создайте private security advisory в GitHub или свяжитесь с автором через
профиль https://github.com/RoBiN19666.
