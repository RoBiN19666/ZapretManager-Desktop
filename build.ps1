# Build ZapretManager.exe with the in-box .NET Framework compiler (csc.exe).
# Requirements: Windows 10/11 only - no SDK, NuGet or installs needed.
[CmdletBinding()]
param(
    [string]$OutDir = "",
    [switch]$Quiet,
    [switch]$SkipIcon
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$src = Join-Path $root "src"
$assets = Join-Path $root "assets"
if (-not $OutDir) { $OutDir = $root }

$csc = Join-Path $env:WINDIR "Microsoft.NET\Framework64\v4.0.30319\csc.exe"
if (-not (Test-Path $csc)) {
    $csc = Join-Path $env:WINDIR "Microsoft.NET\Framework\v4.0.30319\csc.exe"
}
if (-not (Test-Path $csc)) { throw "csc.exe (.NET Framework 4.x) not found. Install .NET Framework 4.8." }

$icon = Join-Path $assets "zapret-manager.ico"
if (-not $SkipIcon -and -not (Test-Path $icon)) {
    $py = (Get-Command python -ErrorAction SilentlyContinue)
    if ($py) {
        & $py.Source (Join-Path $root "tools\make_icon.py")
    } else {
        Write-Warning "Python not found, building without icon."
    }
}

$fw = Split-Path -Parent $csc
$refs = @(
    "System.dll", "System.Core.dll", "System.Drawing.dll", "System.Windows.Forms.dll",
    "System.Management.dll", "System.ServiceProcess.dll", "System.Web.Extensions.dll",
    "System.IO.Compression.dll", "System.IO.Compression.FileSystem.dll"
)

$sources = Get-ChildItem -Path $src -Filter *.cs | Sort-Object Name | ForEach-Object { $_.FullName }
if ($sources.Count -eq 0) { throw "No .cs files in src." }

$out = Join-Path $OutDir "ZapretManager.exe"
$cscArgs = @()
$cscArgs += "/nologo"
$cscArgs += "/codepage:65001"
$cscArgs += "/target:winexe"
$cscArgs += "/platform:anycpu"
$cscArgs += "/optimize+"
$cscArgs += "/debug-"
$cscArgs += "/out:`"$out`""
$cscArgs += "/win32manifest:`"$(Join-Path $src 'app.manifest')`""
if (Test-Path $icon) { $cscArgs += "/win32icon:`"$icon`"" }
foreach ($res in @("app-icon.png", "logo-wide.png", "lock.png", "boosty.png", "donationalerts.png", "tray.ico")) {
    $resPath = Join-Path $assets $res
    if (Test-Path $resPath) { $cscArgs += "/resource:`"$resPath`"" }
}
foreach ($r in $refs) { $cscArgs += "/r:`"$(Join-Path $fw $r)`"" }
foreach ($s in $sources) { $cscArgs += "`"$s`"" }

# --- Версия файла: берём из AppInfo.Version, иначе самообновление видит старую метку ---
$corePath = Join-Path $src "Core.cs"
$coreText = Get-Content $corePath -Raw
$full = ""
$mv = [regex]::Match($coreText, 'Version\s*=\s*"([^"]+)"')
if ($mv.Success) {
    $full = $mv.Groups[1].Value
    $numeric = [regex]::Match($full, '^\d+(\.\d+){0,3}')
    $parts = @($numeric.Value.Split('.') | ForEach-Object { [int]$_ })
    while ($parts.Count -lt 4) { $parts += 0 }
    $fileVersion = ($parts -join '.')
    $genDir = Join-Path $root "build"
    if (-not (Test-Path $genDir)) { New-Item -ItemType Directory -Path $genDir | Out-Null }
    $genFile = Join-Path $genDir "VersionInfo.cs"
    $genLines = @(
        'using System.Reflection;',
        ('[assembly: AssemblyTitle("Zapret Manager \u2014 \u043e\u0431\u0445\u043e\u0434 DPI")]'),
        ('[assembly: AssemblyDescription("Zapret Manager \u2014 \u043e\u0431\u0445\u043e\u0434 DPI \u0434\u043b\u044f Discord \u0438 YouTube")]'),
        ('[assembly: AssemblyProduct("Zapret Manager")]'),
        ('[assembly: AssemblyCompany("RoBiN19666")]'),
        ('[assembly: AssemblyVersion("' + $fileVersion + '")]'),
        ('[assembly: AssemblyFileVersion("' + $fileVersion + '")]'),
        ('[assembly: AssemblyInformationalVersion("' + $full + '")]')
    )
    Set-Content -Path $genFile -Value $genLines -Encoding UTF8
    $cscArgs += ('"' + $genFile + '"')
    if (-not $Quiet) { Write-Host ("file version: " + $fileVersion + " (" + $full + ")") -ForegroundColor DarkGray }
}

if (-not $Quiet) { Write-Host "Compiling $($sources.Count) files -> $out" }

$tmp = [System.IO.Path]::GetTempFileName()
$errFile = $tmp + ".err"
$proc = Start-Process -FilePath $csc -ArgumentList $cscArgs -NoNewWindow -Wait -PassThru -RedirectStandardOutput $tmp -RedirectStandardError $errFile
$stdout = ""
$stderr = ""
if (Test-Path $tmp) { $stdout = Get-Content $tmp -Raw -ErrorAction SilentlyContinue }
if (Test-Path $errFile) { $stderr = Get-Content $errFile -Raw -ErrorAction SilentlyContinue }
Remove-Item $tmp -Force -ErrorAction SilentlyContinue
Remove-Item $errFile -Force -ErrorAction SilentlyContinue

if ($proc.ExitCode -ne 0) {
    if ($stdout) { Write-Host $stdout }
    if ($stderr) { Write-Host $stderr }
    throw "Build failed (exit $($proc.ExitCode))."
}
if (-not $Quiet) {
    $size = [math]::Round((Get-Item $out).Length / 1KB, 1)
    Write-Host "OK: $out ($size KB)" -ForegroundColor Green
}

# --- Пакет для релиза: отдельная папка dist со сборкой и контрольной суммой ---
$verFile = Join-Path $root "version.txt"
$version = if ($full.Length -gt 0) { $full } elseif (Test-Path $verFile) { (Get-Content $verFile -Raw).Trim() } else { "0.0.0" }
$dist = Join-Path $root "dist"
if (-not (Test-Path $dist)) { New-Item -ItemType Directory -Path $dist | Out-Null }
$distExe = Join-Path $dist "ZapretManager.exe"
Copy-Item $out $distExe -Force
$hash = (Get-FileHash $distExe -Algorithm SHA256).Hash.ToLower()
$checks = "$hash  ZapretManager.exe`r`n"
Set-Content -Path (Join-Path $dist "checksums.txt") -Value $checks -Encoding ASCII -NoNewline
if (-not $Quiet) {
    Write-Host "dist: $distExe ($([math]::Round((Get-Item $distExe).Length / 1KB, 1)) KB, версия $version)" -ForegroundColor DarkGray
    Write-Host "SHA-256: $hash" -ForegroundColor DarkGray
}