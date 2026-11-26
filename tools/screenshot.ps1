# Captures the Zapret Manager window into a PNG (for visual verification).
param(
    [string]$ProcessName = "ZapretManager",
    [string]$Out = "tools\shot.png"
)

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class WinApi2 {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
}
"@

[WinApi2]::SetProcessDPIAware() | Out-Null

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { throw "Process '$ProcessName' with a window not found" }
$h = $proc.MainWindowHandle
[WinApi2]::ShowWindow($h, 9) | Out-Null
[WinApi2]::BringWindowToTop($h) | Out-Null
[WinApi2]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 900

$r = New-Object WinApi2+RECT
[WinApi2]::GetWindowRect($h, [ref]$r) | Out-Null
$w = $r.Right - $r.Left
$hh = $r.Bottom - $r.Top
Write-Host "window rect: $($r.Left),$($r.Top) ${w}x${hh}"

$bmp = New-Object System.Drawing.Bitmap ($w), ($hh)
$g = [System.Drawing.Graphics]::FromImage($bmp)
try {
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size ($w), ($hh)))
} catch {
    Write-Host "CopyFromScreen failed: $($_.Exception.Message)"
}
$g.Dispose()
$dir = Split-Path -Parent $Out
if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
$full = Join-Path (Get-Location).Path $Out
$bmp.Save($full, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Host "saved $full"
