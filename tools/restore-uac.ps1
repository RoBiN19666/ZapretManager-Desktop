# Возвращает стандартные запросы подтверждения UAC (отменяет tools\relax-uac.ps1).
# Запускать от имени администратора.

$key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
$backupDir = Join-Path $env:ProgramData 'ZapretManager'
$backup = Join-Path $backupDir 'uac-backup.json'

$consent = 5
$secureDesktop = 1
if (Test-Path $backup) {
    try {
        $data = Get-Content $backup -Raw | ConvertFrom-Json
        if ($null -ne $data.ConsentPromptBehaviorAdmin) { $consent = [int]$data.ConsentPromptBehaviorAdmin }
        if ($null -ne $data.PromptOnSecureDesktop) { $secureDesktop = [int]$data.PromptOnSecureDesktop }
    } catch { }
}

Set-ItemProperty -Path $key -Name ConsentPromptBehaviorAdmin -Value $consent -Type DWord
Set-ItemProperty -Path $key -Name PromptOnSecureDesktop -Value $secureDesktop -Type DWord

$after = Get-ItemProperty -Path $key
@{
    ConsentPromptBehaviorAdmin = $after.ConsentPromptBehaviorAdmin
    PromptOnSecureDesktop = $after.PromptOnSecureDesktop
} | ConvertTo-Json | Write-Output
