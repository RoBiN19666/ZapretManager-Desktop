# Временно отключает запрос подтверждения UAC для администраторов
# (ConsentPromptBehaviorAdmin=0, PromptOnSecureDesktop=0), сохраняя резервную копию.
# Запускать от имени администратора. Обратная операция — tools\restore-uac.ps1.

$key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
$backupDir = Join-Path $env:ProgramData 'ZapretManager'
if (-not (Test-Path $backupDir)) { New-Item -ItemType Directory -Path $backupDir -Force | Out-Null }
$backup = Join-Path $backupDir 'uac-backup.json'

$current = Get-ItemProperty -Path $key
$data = @{
    EnableLUA = $current.EnableLUA
    ConsentPromptBehaviorAdmin = $current.ConsentPromptBehaviorAdmin
    PromptOnSecureDesktop = $current.PromptOnSecureDesktop
    SavedAt = (Get-Date).ToString('s')
}
$data | ConvertTo-Json | Out-File -FilePath $backup -Encoding UTF8

Set-ItemProperty -Path $key -Name ConsentPromptBehaviorAdmin -Value 0 -Type DWord
Set-ItemProperty -Path $key -Name PromptOnSecureDesktop -Value 0 -Type DWord

$after = Get-ItemProperty -Path $key
$result = @{
    backup = $backup
    before = $data
    after = @{
        ConsentPromptBehaviorAdmin = $after.ConsentPromptBehaviorAdmin
        PromptOnSecureDesktop = $after.PromptOnSecureDesktop
    }
}
$result | ConvertTo-Json -Depth 4 | Out-File -FilePath (Join-Path $backupDir 'uac-result.json') -Encoding UTF8
Write-Output ($result | ConvertTo-Json -Depth 4)
