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
foreach ($res in @("app-icon.png", "logo-wide.png")) {
    $resPath = Join-Path $assets $res
    if (Test-Path $resPath) { $cscArgs += "/resource:`"$resPath`"" }
}
foreach ($r in $refs) { $cscArgs += "/r:`"$(Join-Path $fw $r)`"" }
foreach ($s in $sources) { $cscArgs += "`"$s`"" }

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
