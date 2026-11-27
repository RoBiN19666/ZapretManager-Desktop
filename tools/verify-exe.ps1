# Проверка собранного ZapretManager.exe перед публикацией.
#
# Используется в CI (release.yml) и локально: tools\verify-exe.ps1 dist\ZapretManager.exe
# Проверяет то, из-за чего чаще всего ругаются антивирусы и SmartScreen:
#   * в exe встроен манифест (asInvoker) — иначе Windows применяет "installer detection";
#   * есть ресурс версии (компания, продукт, описание) — без него файл выглядит "свежесобранным";
#   * нет упаковщика UPX — он сам по себе сильный триггер эвристик;
#   * печатает SHA-256 и состояние подписи Authenticode.
#
# Код возврата 1 — критичная проблема (нет манифеста или версии).

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Path,

    # В релизной ветке подпись обязательна: без неё скрипт падает.
    [switch]$RequireSignature
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path -LiteralPath $Path)) {
    Write-Host "FAIL: file not found: $Path" -ForegroundColor Red
    exit 1
}

$item = Get-Item -LiteralPath $Path
$bytes = [System.IO.File]::ReadAllBytes($item.FullName)
$ascii = [System.Text.Encoding]::ASCII.GetString($bytes)
$utf16 = [System.Text.Encoding]::Unicode.GetString($bytes)

$problems = New-Object System.Collections.Generic.List[string]

# 1. PE-заголовок
if ($bytes.Length -lt 2 -or $bytes[0] -ne 0x4D -or $bytes[1] -ne 0x5A) {
    $problems.Add('not a PE executable (no MZ header)')
}

# 2. Манифест
$hasManifest = $ascii.Contains('requestedExecutionLevel') -or $utf16.Contains('requestedExecutionLevel')
$asInvoker = $ascii.Contains('asInvoker') -or $utf16.Contains('asInvoker')
if (-not $hasManifest) {
    $problems.Add('embedded manifest with requestedExecutionLevel not found')
} elseif (-not $asInvoker) {
    Write-Host 'WARN: manifest has no asInvoker level' -ForegroundColor Yellow
}

# 3. Ресурс версии
$vi = $item.VersionInfo
if ([string]::IsNullOrWhiteSpace($vi.FileVersion)) {
    $problems.Add('VERSIONINFO resource is missing (FileVersion is empty)')
}
if ([string]::IsNullOrWhiteSpace($vi.CompanyName)) {
    $problems.Add('VERSIONINFO resource is missing (CompanyName is empty)')
}

# 4. Упаковщик
$upx = $ascii.Contains('UPX0') -or $ascii.Contains('UPX1')
if ($upx) {
    Write-Host 'WARN: UPX sections found — packers are a strong false-positive trigger' -ForegroundColor Yellow
}

# 5. Подпись и хэш
$sig = Get-AuthenticodeSignature -LiteralPath $item.FullName
$hash = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash

if ($RequireSignature -and $sig.Status -ne 'Valid') {
    $problems.Add("Authenticode signature is required but status is '$($sig.Status)'")
}

Write-Host ''
Write-Host ("File:      {0}" -f $item.FullName)
Write-Host ("Size:      {0} bytes" -f $bytes.Length)
Write-Host ("Version:   {0}" -f $vi.FileVersion)
Write-Host ("Company:   {0}" -f $vi.CompanyName)
Write-Host ("Product:   {0}" -f $vi.ProductName)
Write-Host ("Manifest:  {0}" -f $(if ($hasManifest) { 'yes' } else { 'NO' }))
Write-Host ("UPX:       {0}" -f $(if ($upx) { 'yes' } else { 'no' }))
Write-Host ("Signature: {0}" -f $sig.Status)
if ($sig.SignerCertificate) {
    Write-Host ("Signer:    {0}" -f $sig.SignerCertificate.Subject)
}
Write-Host ("SHA-256:   {0}" -f $hash)
Write-Host ''

if ($problems.Count -gt 0) {
    foreach ($p in $problems) {
        Write-Host ("FAIL: {0}" -f $p) -ForegroundColor Red
    }
    exit 1
}

Write-Host 'OK: executable metadata looks sane.' -ForegroundColor Green
exit 0
