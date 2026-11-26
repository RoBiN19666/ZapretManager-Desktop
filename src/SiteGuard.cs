using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Diagnostics.Eventing.Reader;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;

namespace ZapretManager
{
    /// <summary>
    /// Фоновая слежка за блокировками: читает журнал DNS-клиента Windows, берёт имена,
    /// которые запрашивали браузеры, быстро проверяет их и добавляет заблокированные в списки.
    /// </summary>
    public static class SiteGuard
    {
        private const string LogName = "Microsoft-Windows-DNS-Client/Operational";
        private static readonly string[] Browsers = new string[]
        {
            "chrome", "msedge", "firefox", "opera", "brave", "vivaldi", "yandex", "browser", "thorium", "librewolf"
        };

        private static long _lastRecordId = -1;
        private static readonly object _stateLock = new object();
        /// <summary>Служебный режим: учитывать процессы любых приложений, а не только браузеров.</summary>
        public static volatile bool AllowAllProcesses;
        /// <summary>Сколько первых событий писать в журнал (для отладки).</summary>
        public static int DebugFirst;
        private static readonly Dictionary<string, DateTime> _checked = new Dictionary<string, DateTime>(StringComparer.OrdinalIgnoreCase);

        /// <summary>Журнал DNS-клиента включён?</summary>
        public static bool LogEnabled()
        {
            try
            {
                EventLogQuery q = new EventLogQuery(LogName, PathType.LogName);
                using (EventLogReader r = new EventLogReader(q)) { r.ReadEvent(); }
                return true;
            }
            catch { return false; }
        }

        /// <summary>Включает журнал (нужны права администратора).</summary>
        public static string EnableLog()
        {
            try
            {
                Proc.Run("wevtutil.exe", "sl \"" + LogName + "\" /e:true", 15000);
                if (LogEnabled()) return "Журнал DNS-клиента включён.";
                return "Не удалось включить журнал (нужны права администратора).";
            }
            catch (Exception ex)
            {
                return "Не удалось включить журнал: " + ex.Message;
            }
        }

        /// <summary>Сбрасывает курсор: при старте историю не разбираем.</summary>
        public static void ResetCursor()
        {
            lock (_stateLock)
            {
                _lastRecordId = -1;
                _checked.Clear();
            }
        }

        /// <summary>Последние имена из журнала DNS, которые запрашивали браузеры.</summary>
        public static List<string> NewBrowserHosts(out string error, out int scanned)
        {
            return NewBrowserHosts(null, out error, out scanned);
        }

        /// <summary>То же, но с фильтром по конкретным PID (для «Проверки приложения»).</summary>
        public static List<string> NewBrowserHosts(ISet<int> allowedPids, out string error, out int scanned)
        {
            lock (_stateLock)
            {
            error = "";
            scanned = 0;
            List<string> hosts = new List<string>();
            try
            {
                StringBuilder xpath = new StringBuilder("*[System[(EventID=3020 or EventID=3022 or EventID=3021)]]");
                EventLogQuery q = new EventLogQuery(LogName, PathType.LogName, xpath.ToString());
                q.ReverseDirection = true;
                long maxSeen = _lastRecordId;
                List<EventRecord> batch = new List<EventRecord>();
                using (EventLogReader reader = new EventLogReader(q))
                {
                    for (int i = 0; i < 200; i++)
                    {
                        EventRecord rec;
                        try { rec = reader.ReadEvent(); }
                        catch { break; }
                        if (rec == null) break;
                        if (_lastRecordId >= 0 && rec.RecordId <= _lastRecordId) { rec.Dispose(); break; }
                        long rid = rec.RecordId ?? 0;
                        if (rid > maxSeen) maxSeen = rid;
                        batch.Add(rec);
                    }
                }

                foreach (EventRecord rec in batch)
                {
                    try
                    {
                        scanned++;
                        // журнал локализован, поэтому домен ищем в тексте, а ClientPID — в XML события
                        string message = rec.FormatDescription() ?? "";
                        int clientPid = 0;
                        try
                        {
                            string xml = rec.ToXml() ?? "";
                            Match pm = Regex.Match(xml, "<Data Name=['\"]ClientPID['\"]>(\\d+)</Data>");
                            if (pm.Success) int.TryParse(pm.Groups[1].Value, out clientPid);
                        }
                        catch { }
                        if (DebugFirst > 0 && scanned <= DebugFirst)
                        {
                            string preview = message.Replace("\r", " ").Replace("\n", " ");
                            Log.Info("Сторож debug: id=" + rec.Id + " pid=" + clientPid + " len=" + message.Length + " [" + (preview.Length > 240 ? preview.Substring(0, 240) : preview) + "]");
                        }
                        Match nameMatch = Regex.Match(message, "([a-z0-9](?:[a-z0-9-]*[a-z0-9])?\\.)+[a-z]{2,24}", RegexOptions.IgnoreCase);
                        Match pidMatch = Regex.Match(message, "(?i)(?:client\\s+pid|pid|process|процесс[^0-9]{0,12})\\s*[:=]?\\s*(\\d{2,6})");
                        if (!nameMatch.Success) continue;
                        string host = nameMatch.Value.Trim('.').ToLowerInvariant();
                        if (DebugFirst > 0 && scanned <= DebugFirst)
                        {
                            Log.Info("Сторож debug: match=" + nameMatch.Success + " host=[" + host + "] len=" + host.Length + " dot=" + host.Contains(".") + " noise=" + IsNoise(host));
                        }
                        if (host.Length < 4 || host.Length > 120) continue;
                        if (!host.Contains(".")) continue;
                        if (host.EndsWith(".local") || host.EndsWith(".lan") || host.EndsWith(".arpa")) continue;
                        if (host.StartsWith("wpad") || host == "localhost") continue;
                        if (IsNoise(host)) continue;

                        if (allowedPids != null)
                        {
                            // «Проверка приложения»: только процессы выбранного приложения.
                            if (clientPid <= 0 || !allowedPids.Contains(clientPid)) continue;
                        }
                        else if (clientPid > 0)
                        {
                            if (!AllowAllProcesses && !IsBrowser(clientPid)) continue;
                        }
                        else if (pidMatch.Success)
                        {
                            int pid;
                            if (int.TryParse(pidMatch.Groups[1].Value, out pid) && !AllowAllProcesses && !IsBrowser(pid)) continue;
                        }
                        if (!hosts.Contains(host)) hosts.Add(host);
                    }
                    catch { }
                    finally { rec.Dispose(); }
                }
                _lastRecordId = maxSeen;
            }
            catch (Exception ex)
            {
                error = ex.Message;
            }
            return hosts;
            }
        }

        private static readonly string[] NoiseHosts = new string[]
        {
            "microsoft.com", "windowsupdate.com", "windows.com", "msftncsi.com", "msedge.net", "bing.com",
            "live.com", "office.com", "office365.com", "outlook.com", "xboxlive.com", "microsoftonline.com",
            "gstatic.com", "google.com", "googleapis.com", "googlevideo.com", "ytimg.com", "googleusercontent.com",
            "cloudflare.com", "cloudflare-dns.com", "akamai.net", "akamaized.net", "cloudfront.net",
            "apple.com", "icloud.com", "mozilla.org", "mozilla.net", "sentry.io", "githubusercontent.com",
            "steamstatic.com", "steampowered.com", "epicgames.com", "battle.net", "riotgames.com"
        };

        private static bool IsNoise(string host)
        {
            foreach (string n in NoiseHosts)
            {
                if (host == n || host.EndsWith("." + n, StringComparison.OrdinalIgnoreCase)) return true;
            }
            return false;
        }

        private static bool IsBrowser(int pid)
        {
            try
            {
                using (System.Diagnostics.Process p = System.Diagnostics.Process.GetProcessById(pid))
                {
                    string name = p.ProcessName.ToLowerInvariant();
                    foreach (string b in Browsers)
                    {
                        if (string.Equals(name, b, StringComparison.OrdinalIgnoreCase)) return true;
                    }
                    return false;
                }
            }
            catch { return false; }
        }

        /// <summary>
        /// Быстрая проверка домена: если TLS рвётся (reset/таймаут) — считаем блокировкой.
        /// </summary>
        public static bool LooksBlocked(string host, out string reason)
        {
            reason = "";
            int ms;
            string err;
            if (!SiteDoctor.TcpConnect(host, 443, 2500, out ms, out err))
            {
                reason = "TCP: " + err;
                return err.IndexOf("таймаут", StringComparison.OrdinalIgnoreCase) >= 0
                    || err.IndexOf("reset", StringComparison.OrdinalIgnoreCase) >= 0
                    || err.IndexOf("сброс", StringComparison.OrdinalIgnoreCase) >= 0;
            }
            int tlsMs;
            string tlsError;
            if (!SiteDoctor.TlsHandshake(host, 4000, out tlsMs, out tlsError))
            {
                reason = "TLS: " + tlsError;
                string e = (tlsError ?? "").ToLowerInvariant();
                return e.Contains("reset") || e.Contains("сброс") || e.Contains("таймаут")
                    || e.Contains("eof") || e.Contains("forcibly") || e.Contains("закрыт");
            }
            return false;
        }

        /// <summary>Уже покрыт списками zapret? (сам домен или его родитель)</summary>
        public static bool AlreadyListed(ZapretInstall install, string host)
        {
            if (install == null || !install.IsValid) return false;
            List<string> entries = new List<string>();
            try
            {
                foreach (string file in Directory.GetFiles(install.ListsDir, "*list*.txt"))
                {
                    if (file.IndexOf("exclude", StringComparison.OrdinalIgnoreCase) >= 0) continue;
                    foreach (string line in TextFile.ReadLines(file))
                    {
                        string norm = ListAudit.Normalize(line);
                        if (norm.Length > 0) entries.Add(norm.StartsWith("^") ? norm.Substring(1) : norm);
                    }
                }
            }
            catch { return false; }
            string h = host;
            while (h.Length > 0)
            {
                foreach (string e in entries)
                {
                    if (string.Equals(e, h, StringComparison.OrdinalIgnoreCase)) return true;
                }
                int dot = h.IndexOf('.');
                if (dot < 0) break;
                h = h.Substring(dot + 1);
                if (h.IndexOf('.') < 0) break;
            }
            return false;
        }

        /// <summary>Проверяет пачку доменов и добавляет заблокированные в пользовательский список.</summary>
        public static List<string> Process(ZapretInstall install, AppConfig config, IEnumerable<string> hosts, bool autoAdd, Action<string> onFound)
        {
            List<string> added = new List<string>();
            if (install == null || !install.IsValid) return added;
            int perCycle = 0;
            foreach (string raw in hosts)
            {
                string host = (raw ?? "").Trim().ToLowerInvariant();
                if (host.Length == 0) continue;
                if (perCycle >= 5) break;

                bool recentlyChecked;
                lock (_stateLock)
                {
                    DateTime last;
                    recentlyChecked = _checked.TryGetValue(host, out last) && (DateTime.Now - last).TotalMinutes < 60;
                    if (!recentlyChecked) _checked[host] = DateTime.Now;
                }
                if (recentlyChecked) continue;
                if (AlreadyListed(install, host)) continue;

                perCycle++;
                string reason;
                if (!LooksBlocked(host, out reason)) continue;

                Log.Warn("Сторож: " + host + " похоже заблокирован (" + reason + ").");
                if (autoAdd)
                {
                    int count;
                    string path;
                    string message = SiteDoctor.AddToUserList(install, new string[] { host }, out count, out path);
                    if (count > 0)
                    {
                        added.Add(host);
                        Log.Ok("Сторож: " + host + " добавлен в " + Path.GetFileName(path) + " (" + reason + ").");
                    }
                }
                if (onFound != null) onFound(host + " — " + reason + (autoAdd ? " (добавлен в список)" : " (требуется добавление)"));
            }
            return added;
        }

        /// <summary>Убирает старые записи из кэша проверок, чтобы он не рос.</summary>
        public static void Trim()
        {
            lock (_stateLock)
            {
                if (_checked.Count < 500) return;
                List<string> stale = new List<string>();
                foreach (KeyValuePair<string, DateTime> pair in _checked)
                {
                    if ((DateTime.Now - pair.Value).TotalHours > 6) stale.Add(pair.Key);
                }
                foreach (string key in stale) _checked.Remove(key);
            }
        }
    }
}
