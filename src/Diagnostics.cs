using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.ServiceProcess;
using System.Text;
using System.Text.RegularExpressions;
using Microsoft.Win32;

namespace ZapretManager
{
    public enum CheckSeverity { Ok, Warn, Error }

    public sealed class CheckResult
    {
        public string Name = "";
        public CheckSeverity Severity;
        public string Message = "";
        public string Hint = "";
        public string ActionId = "";
        public string ActionText = "";
        public string Group = "";

        public string SeverityText
        {
            get
            {
                if (Severity == CheckSeverity.Error) return "ОШИБКА";
                if (Severity == CheckSeverity.Warn) return "ВНИМАНИЕ";
                return "OK";
            }
        }
    }

    public static class Diagnostics
    {
        public const string ActionRemoveWinDivert = "windivert";
        public const string ActionRemoveConflicts = "conflicts";
        public const string ActionEnableTcp = "tcp";
        public const string ActionClearDiscord = "discord";
        public const string ActionCleanHosts = "hosts";

        public static List<CheckResult> Run(ZapretInstall install, ZapretOps ops, AppConfig config, Action<string> progress)
        {
            List<CheckResult> list = new List<CheckResult>();
            Dictionary<string, ServiceControllerStatus> services = null;
            try
            {
                services = new Dictionary<string, ServiceControllerStatus>(StringComparer.OrdinalIgnoreCase);
                foreach (ServiceController sc in ServiceController.GetServices())
                {
                    try { services[sc.ServiceName] = sc.Status; }
                    catch { }
                    sc.Dispose();
                }
            }
            catch (Exception ex) { Log.Warn("Не удалось получить список служб: " + ex.Message); }

            Step(progress, "Права администратора…");
            Add(list, "Права администратора", Native.IsAdmin() ? CheckSeverity.Ok : CheckSeverity.Error,
                Native.IsAdmin() ? "Приложение запущено с правами администратора." : "Приложение запущено без прав администратора — запуск bypass и управление службами недоступны.",
                "Запустите Zapret Manager от имени администратора.", "", "", "Система");

            Step(progress, "Установка zapret…");
            if (install.IsValid)
            {
                Add(list, "Файлы zapret", CheckSeverity.Ok, "Найдена установка версии " + install.Version + " в " + install.Root, "", "", "", "Система");
                bool sysExists = Directory.Exists(install.BinDir) && Directory.GetFiles(install.BinDir, "*.sys").Length > 0;
                Add(list, "Драйвер WinDivert", sysExists ? CheckSeverity.Ok : CheckSeverity.Error,
                    sysExists ? "WinDivert64.sys на месте." : "Файл WinDivert64.sys не найден в папке bin.",
                    "Без драйвера bypass не работает. Обновите или переустановите zapret.", "", "", "Система");
                bool cyr = Regex.IsMatch(install.Root, "[\u0430-\u044F\u0410-\u042F\u0451\u0401]");
                Add(list, "Путь установки", cyr ? CheckSeverity.Warn : CheckSeverity.Ok,
                    cyr ? "Путь содержит кириллицу: " + install.Root : "Путь без кириллицы — " + install.Root,
                    "Если bypass не работает, перенесите папку, например, в C:\\zapret.", "", "", "Система");
                string oneDrive = Environment.GetEnvironmentVariable("OneDrive");
                if (!string.IsNullOrEmpty(oneDrive) && install.Root.StartsWith(oneDrive, StringComparison.OrdinalIgnoreCase))
                {
                    Add(list, "OneDrive", CheckSeverity.Error, "zapret установлен в папку OneDrive.", "Перенесите папку из OneDrive, например в C:\\zapret.", "", "", "Система");
                }
                else Add(list, "OneDrive", CheckSeverity.Ok, "Установка не находится в OneDrive.", "", "", "", "Система");
            }
            else
            {
                Add(list, "Файлы zapret", CheckSeverity.Error, "Папка zapret не найдена: нет service.bat или bin\\winws.exe.",
                    "Укажите папку zapret вручную на странице «Обзор».", "", "", "Система");
            }

            Step(progress, "Службы Windows…");
            AddServiceCheck(list, services, "BFE", "Base Filtering Engine", true,
                "Базовый фильтрующий движок Windows остановлен. Он обязателен для работы zapret.");

            Step(progress, "Состояние bypass…");
            if (install.IsValid)
            {
                ServiceSnapshot svc = ZapretOps.QueryService("zapret");
                ServiceSnapshot wd = ZapretOps.QueryService("WinDivert");
                bool winws = Proc.AnyRunning("winws.exe");
                if (winws)
                {
                    Add(list, "Процесс winws.exe", CheckSeverity.Ok, "winws.exe запущен.", "", "", "", "Bypass");
                }
                else if (!svc.Installed)
                {
                    Add(list, "Процесс winws.exe", CheckSeverity.Warn, "Bypass не запущен.", "Запустите стратегию на странице «Стратегии».", "", "", "Bypass");
                }
                if (svc.Installed)
                {
                    Add(list, "Служба zapret", svc.Status == "работает" ? CheckSeverity.Ok : CheckSeverity.Warn,
                        "Служба установлена, состояние: " + svc.Status + (svc.Strategy.Length > 0 ? ", стратегия: " + svc.Strategy : ""), "", "", "", "Bypass");
                    if (ops.ServicePointsElsewhere)
                    {
                        Add(list, "Папка службы", CheckSeverity.Warn,
                            "Служба zapret установлена из другой папки: " + ops.ServiceInstallRoot,
                            "Менеджер управляет папкой " + install.Root + ". Переустановите службу из нужной папки (страница «Обзор»).", "", "", "Bypass");
                    }
                    else if (ops.ServicePathMissing)
                    {
                        Add(list, "Папка службы", CheckSeverity.Error,
                            "Служба zapret указывает на несуществующую папку: " + Convert.ToString(ops.ServiceInstallRoot),
                            "После перезагрузки bypass не запустится. Переустановите службу из текущей папки.", "", "", "Bypass");
                    }
                }
                if (!winws && wd.Installed)
                {
                    Add(list, "Зависший драйвер WinDivert", CheckSeverity.Error,
                        "Служба WinDivert активна, но winws.exe не запущен — драйвер занят и блокирует запуск.",
                        "Удалите зависший драйвер WinDivert.", ActionRemoveWinDivert, "Удалить WinDivert", "Bypass");
                }
                AddIpSetCheck(list, ops);
            }

            Step(progress, "Системный прокси…");
            try
            {
                int enabled = 0;
                string server = "";
                using (RegistryKey k = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Internet Settings"))
                {
                    if (k != null)
                    {
                        object e = k.GetValue("ProxyEnable");
                        if (e != null) enabled = Convert.ToInt32(e);
                        object s = k.GetValue("ProxyServer");
                        if (s != null) server = Convert.ToString(s);
                    }
                }
                if (enabled == 1)
                    Add(list, "Системный прокси", CheckSeverity.Warn, "Системный прокси включён: " + server,
                        "Убедитесь, что прокси рабочий, или отключите его — иначе Discord/YouTube могут не работать.", "", "", "Система");
                else
                    Add(list, "Системный прокси", CheckSeverity.Ok, "Системный прокси не используется.", "", "", "", "Система");
            }
            catch { }

            Step(progress, "TCP timestamps…");
            if (install.IsValid)
            {
                bool? ts = ZapretOps.TcpTimestampsEnabled();
                if (!ts.HasValue)
                    Add(list, "TCP timestamps", CheckSeverity.Ok, "Состояние TCP timestamps определить не удалось (не критично).", "", "", "", "Сеть");
                else if (ts.Value)
                    Add(list, "TCP timestamps", CheckSeverity.Ok, "TCP timestamps включены.", "", "", "", "Сеть");
                else
                    Add(list, "TCP timestamps", CheckSeverity.Warn, "TCP timestamps выключены — часть стратегий без них не работает.",
                        "Включите параметр (кнопка ниже).", ActionEnableTcp, "Включить timestamps", "Сеть");
            }

            Step(progress, "Конфликтующие программы…");
            try
            {
                if (Proc.AnyRunning("AdguardSvc.exe"))
                    Add(list, "Adguard", CheckSeverity.Error, "Найден процесс AdguardSvc.exe.", "Adguard конфликтует с Discord при работе zapret. Закройте его.", "", "", "Конфликты");
                else Add(list, "Adguard", CheckSeverity.Ok, "Adguard не запущен.", "", "", "", "Конфликты");
            }
            catch { }

            if (services != null)
            {
                List<string> killer = FindServices(services, "Killer");
                Add(list, "Killer (Rivet Networks)", killer.Count > 0 ? CheckSeverity.Error : CheckSeverity.Ok,
                    killer.Count > 0 ? "Найдены службы Killer: " + Join(killer) : "Служба Killer не найдена.",
                    "Killer конфликтует с zapret — отключите его.", "", "", "Конфликты");

                List<string> intel = FindServices(services, "Intel", "Connectivity", "Network");
                Add(list, "Intel Connectivity", intel.Count > 0 ? CheckSeverity.Error : CheckSeverity.Ok,
                    intel.Count > 0 ? "Найдены службы Intel Connectivity: " + Join(intel) : "Служба Intel Connectivity не найдена.",
                    "Известный конфликт с zapret.", "", "", "Конфликты");

                List<string> cp = new List<string>();
                cp.AddRange(FindServices(services, "TracSrvWrapper"));
                cp.AddRange(FindServices(services, "EPWD"));
                Add(list, "Check Point", cp.Count > 0 ? CheckSeverity.Error : CheckSeverity.Ok,
                    cp.Count > 0 ? "Найдены службы Check Point: " + Join(cp) : "Check Point не найден.",
                    "Check Point конфликтует с zapret, попробуйте удалить.", "", "", "Конфликты");

                List<string> smart = FindServices(services, "SmartByte");
                Add(list, "SmartByte", smart.Count > 0 ? CheckSeverity.Error : CheckSeverity.Ok,
                    smart.Count > 0 ? "Найдены службы SmartByte: " + Join(smart) : "SmartByte не найден.",
                    "SmartByte конфликтует с zapret — отключите службу.", "", "", "Конфликты");

                List<string> vpn = new List<string>();
                vpn.AddRange(FindServices(services, "VPN"));
                vpn.AddRange(FindServices(services, "HappService"));
                Add(list, "VPN", vpn.Count > 0 ? CheckSeverity.Warn : CheckSeverity.Ok,
                    vpn.Count > 0 ? "Найдены VPN-службы: " + Join(vpn) : "VPN-служб не найдено.",
                    "Некоторые VPN конфликтуют с zapret — отключите их на время проверки.", "", "", "Конфликты");

                List<string> conflicts = new List<string>();
                foreach (string name in new string[] { "GoodbyeDPI", "discordfix_zapret", "winws1", "winws2" })
                {
                    ServiceControllerStatus st;
                    if (services.TryGetValue(name, out st)) conflicts.Add(name);
                }
                Add(list, "Другие bypass-службы", conflicts.Count > 0 ? CheckSeverity.Error : CheckSeverity.Ok,
                    conflicts.Count > 0 ? "Найдены конкурирующие службы: " + Join(conflicts) : "Конкурирующих bypass-служб не найдено.",
                    "Остановите и удалите их, иначе два bypass будут мешать друг другу.",
                    conflicts.Count > 0 ? ActionRemoveConflicts : "", conflicts.Count > 0 ? "Удалить конфликты" : "", "Конфликты");

                ServiceControllerStatus bfe;
                if (services.TryGetValue("WinDivert", out bfe) && bfe == ServiceControllerStatus.Running && !Proc.AnyRunning("winws.exe"))
                {
                    Add(list, "Служба WinDivert", CheckSeverity.Warn, "Служба WinDivert запущена без winws.exe.", "Очистите её, чтобы избежать конфликта.", ActionRemoveWinDivert, "Очистить WinDivert", "Конфликты");
                }
            }

            Step(progress, "DNS и hosts…");
            try
            {
                int doh = CountDohInterfaces();
                Add(list, "Шифрованный DNS (DoH)", CheckSeverity.Ok,
                    doh > 0 ? string.Format("Шифрованный DNS настроен в системе ({0} интерф.), проверить нечего.", doh) : "Шифрованный DNS в системе не настроен.",
                    doh > 0 ? "" : "Включите «Шифрованный DNS» в настройках Windows 11 или в браузере — так обход работает стабильнее.", "", "", "Сеть");
            }
            catch { }

            try
            {
                string hosts = ZapretOps.HostsPath;
                if (File.Exists(hosts))
                {
                    string text = File.ReadAllText(hosts).ToLowerInvariant();
                    bool youtube = text.Contains("youtube.com") || text.Contains("youtu.be");
                    bool ourBlock = text.Contains("# >>> zapret-manager");
                    if (youtube)
                        Add(list, "Файл hosts", CheckSeverity.Warn, "В hosts есть записи youtube.com / youtu.be — это может ломать доступ к YouTube.",
                            ourBlock ? "Записи добавлены вручную или другой программой. Проверьте hosts." : "Проверьте файл hosts и удалите эти строки.",
                            ourBlock ? ActionCleanHosts : "", ourBlock ? "Убрать блок менеджера" : "", "Сеть");
                    else
                        Add(list, "Файл hosts", CheckSeverity.Ok, "В hosts нет записей YouTube.", "", "", "", "Сеть");
                }
            }
            catch { }

            Step(progress, "Файлы списков…");
            if (install.IsValid)
                AddReferencedFilesCheck(list, install, ops);

            try
            {
                bool adguardDiscord = false;
                foreach (string d in ZapretOps.DiscordCacheDirs()) { adguardDiscord = true; break; }
                if (adguardDiscord)
                    Add(list, "Кэш Discord", CheckSeverity.Ok, "Найдены установки Discord: " + Join(ZapretOps.DiscordCacheDirs()), "При проблемах со звуком/подключением очистите кэш.", ActionClearDiscord, "Очистить кэш Discord", "Bypass");
                else
                    Add(list, "Кэш Discord", CheckSeverity.Ok, "Установки Discord не найдены.", "", "", "", "Bypass");
            }
            catch { }

            Step(progress, "TG WS Proxy…");
            try
            {
                TgProxyState tg = TgProxy.GetState(config);
                if (!tg.Installed)
                {
                    Add(list, "TG WS Proxy", CheckSeverity.Ok, "Прокси Telegram не установлен (необязательный компонент).",
                        "Установить можно на странице «TG-прокси» или через «Мастер установки».", "", "", "Telegram");
                }
                else
                {
                    Add(list, "TG WS Proxy", CheckSeverity.Ok,
                        string.Format("Установлен: версия {0}, {1}", tg.Version.Length > 0 ? tg.Version : "неизвестна", tg.Dir), "", "", "", "Telegram");
                    if (tg.Running)
                        Add(list, "Прокси Telegram", tg.PortListening ? CheckSeverity.Ok : CheckSeverity.Warn,
                            tg.PortListening
                                ? string.Format("Работает на {0}:{1} (PID {2}), секрет {3}.", tg.Host, tg.Port, tg.Pid, tg.Secret.Length > 0 ? "есть" : "нет")
                                : string.Format("Процесс запущен (PID {0}), но порт {1} не слушается.", tg.Pid, tg.Port),
                            tg.PortListening ? "" : "Проверьте журнал TG WS Proxy и свободен ли порт.", "", "", "Telegram");
                    else
                        Add(list, "Прокси Telegram", CheckSeverity.Ok, "Остановлен.", "", "", "", "Telegram");

                    if (tg.InstanceCount > 2)
                        Add(list, "Экземпляры прокси", CheckSeverity.Warn,
                            string.Format("Запущено несколько копий TG WS Proxy: {0}.", tg.InstanceCount),
                            "Лишние копии мешают друг другу. Нажмите «Остановить», затем «Запустить» на странице «TG-прокси».", "", "", "Telegram");
                    string startupExe = TgProxy.StartupFolderExe;
                    if (!string.IsNullOrEmpty(startupExe))
                        Add(list, "Автозагрузка прокси", CheckSeverity.Warn,
                            "Копия TG WS Proxy лежит в папке «Автозагрузка»: " + startupExe,
                            "Это дублирует автозапуск менеджера. Удалите файл из автозагрузки, если включаете автозапуск Zapret Manager.", "", "", "Telegram");
                    if (!tg.Running && tg.PortListening)
                        Add(list, "Порт " + tg.Port, CheckSeverity.Warn,
                            string.Format("Порт {0} занят другим процессом, хотя TG WS Proxy не запущен.", tg.Port),
                            "Освободите порт или измените его в config.json прокси.", "", "", "Telegram");
                    if (tg.ConfigError.Length > 0)
                        Add(list, "Конфигурация прокси", CheckSeverity.Ok, "config.json: " + tg.ConfigError, "", "", "", "Telegram");
                }
            }
            catch (Exception ex)
            {
                Add(list, "TG WS Proxy", CheckSeverity.Warn, "Не удалось проверить: " + ex.Message, "", "", "", "Telegram");
            }

            Step(progress, "Готово.");
            return list;
        }

        private static void AddIpSetCheck(List<CheckResult> list, ZapretOps ops)
        {
            try
            {
                string mode = ops.GetIpsetMode();
                long lines = ops.CountListLines(ops.Install.IpsetFile);
                long backup = ops.CountListLines(ops.Install.IpsetBackup);
                if (mode == "loaded")
                    Add(list, "IPSet список", lines > 1000 ? CheckSeverity.Ok : CheckSeverity.Warn,
                        string.Format("Загружен полный список: {0} строк.", lines),
                        lines <= 1000 ? "Список подозрительно мал — обновите его на странице «Настройки»." : "", "", "", "Bypass");
                else
                    Add(list, "IPSet список", CheckSeverity.Ok,
                        string.Format("Режим «{0}». Резервный список: {1} строк.", ZapretOps.IpsetModeText(mode), backup), "", "", "", "Bypass");
            }
            catch { }
        }

        private static void AddReferencedFilesCheck(List<CheckResult> list, ZapretInstall install, ZapretOps ops)
        {
            List<string> missing = new List<string>();
            GameFilterState gf = GameFilterState.Load(install.Root);
            foreach (StrategyDef s in install.Strategies)
            {
                string err;
                string[] argv = null;
                try { argv = BatParser.GetArgv(install, s, gf, out err); } catch { }
                if (argv == null) continue;
                foreach (string a in argv)
                {
                    int eq = a.IndexOf('=');
                    if (eq <= 0) continue;
                    string key = a.Substring(0, eq).ToLowerInvariant();
                    if (key != "--hostlist" && key != "--hostlist-exclude" && key != "--ipset" && key != "--ipset-exclude") continue;
                    string value = a.Substring(eq + 1).Trim('"');
                    if (value.Length == 0 || value.IndexOf('%') >= 0) continue;
                    if (!File.Exists(value) && !missing.Contains(Path.GetFileName(value))) missing.Add(Path.GetFileName(value));
                }
            }
            if (missing.Count == 0)
                Add(list, "Файлы списков", CheckSeverity.Ok, "Все списки, на которые ссылаются стратегии, существуют.", "", "", "", "Списки");
            else
                Add(list, "Файлы списков", CheckSeverity.Error, "Не найдены файлы списков: " + Join(missing),
                    "Пересоздайте их: меню «Списки» → «Создать отсутствующие» или обновите zapret.", "", "", "Списки");
        }

        private static int CountDohInterfaces()
        {
            int count = 0;
            try
            {
                using (RegistryKey k = Registry.LocalMachine.OpenSubKey(@"SYSTEM\CurrentControlSet\Services\Dnscache\InterfaceSpecificParameters"))
                {
                    if (k == null) return 0;
                    foreach (string sub in k.GetSubKeyNames())
                    {
                        using (RegistryKey s = k.OpenSubKey(sub))
                        {
                            if (s == null) continue;
                            object v = s.GetValue("DohFlags");
                            if (v != null)
                            {
                                long flags;
                                try { flags = Convert.ToInt64(v); } catch { flags = 0; }
                                if (flags > 0) count++;
                            }
                        }
                    }
                }
            }
            catch { }
            return count;
        }

        private static void AddServiceCheck(List<CheckResult> list, Dictionary<string, ServiceControllerStatus> services, string name, string title, bool required, string problem)
        {
            if (services == null) return;
            ServiceControllerStatus st;
            if (services.TryGetValue(name, out st))
            {
                bool running = st == ServiceControllerStatus.Running;
                Add(list, title, running ? CheckSeverity.Ok : (required ? CheckSeverity.Error : CheckSeverity.Warn),
                    string.Format("Служба {0}: {1}", name, running ? "работает" : "не работает (" + st + ")"),
                    running ? "" : problem, "", "", "Система");
            }
            else
            {
                Add(list, title, required ? CheckSeverity.Error : CheckSeverity.Warn, "Служба " + name + " не найдена.", problem, "", "", "Система");
            }
        }

        private static List<string> FindServices(Dictionary<string, ServiceControllerStatus> services, params string[] parts)
        {
            List<string> found = new List<string>();
            foreach (KeyValuePair<string, ServiceControllerStatus> kv in services)
            {
                bool all = true;
                foreach (string p in parts)
                {
                    if (kv.Key.IndexOf(p, StringComparison.OrdinalIgnoreCase) < 0) { all = false; break; }
                }
                if (all) found.Add(kv.Key);
            }
            return found;
        }

        private static string Join(List<string> items)
        {
            return string.Join(", ", items.ToArray());
        }

        private static void Add(List<CheckResult> list, string name, CheckSeverity sev, string message, string hint, string actionId, string actionText, string group)
        {
            CheckResult c = new CheckResult();
            c.Name = name;
            c.Severity = sev;
            c.Message = message;
            c.Hint = sev == CheckSeverity.Ok ? "" : hint;
            c.ActionId = actionId;
            c.ActionText = actionText;
            c.Group = group;
            list.Add(c);
        }

        private static void Step(Action<string> progress, string text)
        {
            if (progress != null) progress(text);
        }

        public static string RunAction(string actionId, ZapretOps ops)
        {
            switch (actionId)
            {
                case ActionRemoveWinDivert:
                    if (!Native.IsAdmin()) return "Нужны права администратора.";
                    foreach (string n in new string[] { "WinDivert", "WinDivert14" })
                    {
                        Proc.Run("sc.exe", "stop " + n, 12000);
                        Proc.Run("sc.exe", "delete " + n, 12000);
                    }
                    Log.Ok("Драйвер WinDivert остановлен и удалён.");
                    return "Драйвер WinDivert остановлен и удалён.";
                case ActionRemoveConflicts:
                    if (!Native.IsAdmin()) return "Нужны права администратора.";
                    StringBuilder sb = new StringBuilder();
                    foreach (string name in new string[] { "GoodbyeDPI", "discordfix_zapret", "winws1", "winws2" })
                    {
                        ServiceSnapshot s = ZapretOps.QueryService(name);
                        if (s.Installed)
                        {
                            Proc.Run("sc.exe", "stop " + name, 12000);
                            ProcResult r = Proc.Run("sc.exe", "delete " + name, 12000);
                            sb.Append(r.ExitCode == 0 ? "Удалена служба " + name + ". " : "Не удалось удалить " + name + ". ");
                        }
                    }
                    Log.Info("Удаление конфликтующих служб: " + sb);
                    return sb.Length == 0 ? "Конфликтующих служб не найдено." : sb.ToString().Trim();
                case ActionEnableTcp:
                    if (!Native.IsAdmin()) return "Нужны права администратора.";
                    bool ok = ops.EnableTcpTimestamps();
                    Log.Ok(ok ? "TCP timestamps включены." : "Не удалось включить TCP timestamps.");
                    return ok ? "TCP timestamps включены." : "Не удалось включить TCP timestamps.";
                case ActionClearDiscord:
                    return ops.ClearDiscordCache();
                case ActionCleanHosts:
                    return ops.ClearHostsBlock();
                default:
                    return "Действие не поддерживается.";
            }
        }
    }
}
