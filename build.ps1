# Сборка Zapret Manager (Rust).
# Требуется Rust 1.88+ (rustup). Сборка идёт во временную ASCII-папку, потому что
# линковщик MinGW не понимает кириллицу и пробелы в пути к target/.
[CmdletBinding()]
param(
    [string]$OutDir = "",
    [switch]$Quiet
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $OutDir) { $OutDir = Join-Path $root "dist" }

$cargo = (Get-Command cargo -ErrorAction SilentlyContinue)
if (-not $cargo) { throw "cargo не найден. Установите Rust: https://rustup.rs" }

$env:CARGO_TARGET_DIR = Join-Path $env:TEMP "zapret-manager-build"
if (-not $Quiet) { Write-Host "cargo build --release (target: $env:CARGO_TARGET_DIR)" }

Push-Location $root
try {
    & $cargo.Source build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build завершился с кодом $LASTEXITCODE" }
} finally {
    Pop-Location
}

$exe = Join-Path $env:CARGO_TARGET_DIR "release\ZapretManager.exe"
if (-not (Test-Path $exe)) { throw "Не найден $exe" }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$dist = Join-Path $OutDir "ZapretManager.exe"
Copy-Item $exe $dist -Force
$hash = (Get-FileHash $dist -Algorithm SHA256).Hash.ToLower()
"$hash  ZapretManager.exe`r`n" | Set-Content (Join-Path $OutDir "checksums.txt") -Encoding ASCII -NoNewline

$size = [math]::Round((Get-Item $dist).Length / 1MB, 2)
if (-not $Quiet) {
    Write-Host "OK: $dist ($size МБ)" -ForegroundColor Green
    Write-Host "SHA-256: $hash" -ForegroundColor DarkGray
}
