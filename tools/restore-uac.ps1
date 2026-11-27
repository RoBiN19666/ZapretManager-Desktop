# Возвращает стандартные запросы подтверждения UAC (5 / 1), если их отключали вручную.
# Запускать от имени администратора.

#Requires -RunAsAdministrator

$key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
$backupDir = Join-Path $env:ProgramData 'ZapretManager'
$backup = Join-Path $backupDir 'uac-backup.json'

$consent = 5
$secureDesktop = 1

# Значения ConsentPromptBehaviorAdmin: 0 — «повышать без запроса» (UAC фактически
# выключен), 1..5 — разные варианты подтверждения. Ноль из резервной копии НЕ
# восстанавливаем: файл лежит в ProgramData, а этот каталог мог быть создан
# непривилегированным пользователем до установки — иначе получился бы способ
# отключить UAC через подменённый JSON.
$allowedConsent = @(1, 2, 3, 4, 5)

if (Test-Path -LiteralPath $backup) {
    try {
        $data = Get-Content -LiteralPath $backup -Raw | ConvertFrom-Json
        if ($null -ne $data.ConsentPromptBehaviorAdmin) {
            $value = [int]$data.ConsentPromptBehaviorAdmin
            if ($allowedConsent -contains $value) {
                $consent = $value
            } else {
                Write-Warning "uac-backup.json: ConsentPromptBehaviorAdmin=$value недопустим, использую 5."
            }
        }
        if ($null -ne $data.PromptOnSecureDesktop) {
            $value = [int]$data.PromptOnSecureDesktop
            if ($value -eq 0 -or $value -eq 1) {
                $secureDesktop = $value
            } else {
                Write-Warning "uac-backup.json: PromptOnSecureDesktop=$value недопустим, использую 1."
            }
        }
    } catch {
        Write-Warning "uac-backup.json не прочитан ($($_.Exception.Message)), использую 5/1."
    }
}

Set-ItemProperty -Path $key -Name ConsentPromptBehaviorAdmin -Value $consent -Type DWord
Set-ItemProperty -Path $key -Name PromptOnSecureDesktop -Value $secureDesktop -Type DWord

$after = Get-ItemProperty -Path $key
@{
    ConsentPromptBehaviorAdmin = $after.ConsentPromptBehaviorAdmin
    PromptOnSecureDesktop = $after.PromptOnSecureDesktop
} | ConvertTo-Json | Write-Output
