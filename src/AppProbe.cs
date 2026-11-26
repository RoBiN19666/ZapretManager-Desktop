using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Management;
using System.Text.RegularExpressions;

namespace ZapretManager
{
    public sealed class AppEntry
    {
        public int Pid;
        public string Name = "";
        public string Title = "";
        public string Path = "";

        public override string ToString()
        {
            return Name + "  (PID " + Pid + ")" + (Title.Length > 0 ? "  —  " + Title : "");
        }
    }

    /// <summary>
    /// Проверка приложения: собираем домены, к которым оно обращается (журнал DNS), и его адреса (netstat),
    /// проверяем их на блокировку и добавляем найденное в списки zapret.
    /// </summary>
    public static class AppProbe
    {
        private static readonly string[] Noise = new string[]
        {
            "microsoft.com", "windowsupdate.com", "windows.com", "msftncsi.com", "msedge.net", "bing.com",
            "live.com", "office.com", "office365.com", "outlook.com", "xboxlive.com", "microsoftonline.com",
            "gstatic.com", "google.com", "googleapis.com", "googleusercontent.com", "gvt1.com", "gvt2.com",
            "cloudflare.com", "cloudflare-dns.com", "akamai.net", "akamaized.net", "cloudfront.net",
            "apple.com", "icloud.com", "mozilla.org", "mozilla.net", "sentry.io", "digicert.com", "lencr.org",
            "githubusercontent.com", "github.com", "zoom.us", "skype.com", "windows.net"
        };

        /// <summary>Приложения с окнами — то, что пользователь может выбрать.</summary>
        public static List<AppEntry> List()
        {
            List<AppEntry> list = new List<AppEntry>();
            HashSet<string> skip = new HashSet<string>(StringComparer.OrdinalIgnoreCase)
            {
                "explorer", "dwm", "taskhostw", "sihost", "ctfmon", "runtimebroker", "searchapp",
                "startmenuexperiencehost", "shellexperiencehost", "applicationframehost", "textinputhost"
            };
            foreach (Process p in Process.GetProcesses())
            {
                try
                {
                    if (p.Id <= 4) continue;
                    if (skip.Contains(p.ProcessName)) continue;
                    string title = "";
                    try { title = p.MainWindowTitle ?? ""; } catch { }
                    if (title.Length == 0) continue;
                    AppEntry e = new AppEntry();
                    e.Pid = p.Id;
                    e.Name = p.ProcessName;
                    e.Title = title.Length > 60 ? title.Substring(0, 59) + "…" : title;
                    try { e.Path = p.MainModule.FileName; } catch { }
                    list.Add(e);
                }
                catch { }
            }
            list.Sort(delegate(AppEntry a, AppEntry b) { return string.Compare(a.Name, b.Name, StringComparison.OrdinalIgnoreCase); });
            return list;
        }

        /// <summary>Все процессы того же приложения (браузеры и игры дробят работу на подпроцессы).</summary>
        public static List<int> SameApp(int pid)
        {
            List<int> result = new List<int>();
            string path = "";
            string name = "";
            try
            {
                using (Process p = Process.GetProcessById(pid))
                {
                    name = p.ProcessName;
                    try { path = p.MainModule.FileName; } catch { }
                }
            }
            catch { return result; }
            if (name.Length == 0) return result;
            foreach (Process p in Process.GetProcessesByName(name))
            {
                try
                {
                    if (path.Length == 0 || string.Equals(p.MainModule.FileName, path, StringComparison.OrdinalIgnoreCase))
                        result.Add(p.Id);
                }
                catch { }
            }
            return result;
        }

        /// <summary>PID процесса и всех его потомков: лаунчеры запускают игру отдельным процессом.</summary>
        public static List<int> WithChildren(int rootPid)
        {
            List<int> result = new List<int>();
            result.Add(rootPid);
            try
            {
                Dictionary<int, int> parents = new Dictionary<int, int>();
                using (ManagementObjectSearcher s = new ManagementObjectSearcher("SELECT ProcessId, ParentProcessId FROM Win32_Process"))
                using (ManagementObjectCollection col = s.Get())
                {
                    foreach (ManagementBaseObject o in col)
                    {
                        int pid, ppid;
                        if (!int.TryParse(Convert.ToString(o["ProcessId"]), out pid)) continue;
                        if (!int.TryParse(Convert.ToString(o["ParentProcessId"]), out ppid)) continue;
                        parents[pid] = ppid;
                    }
                }
                bool added = true;
                while (added)
                {
                    added = false;
                    foreach (KeyValuePair<int, int> pair in parents)
                    {
                        if (result.Contains(pair.Key)) continue;
                        if (result.Contains(pair.Value)) { result.Add(pair.Key); added = true; }
                    }
                }
            }
            catch { }
            return result;
        }

        /// <summary>Внешние адреса, к которым подключён процесс (netstat).</summary>
        public static List<string> RemoteIps(int pid)
        {
            List<string> ips = new List<string>();
            try
            {
                ProcResult run = Proc.Run("netstat.exe", "-ano -p TCP", 15000);
                string output = run == null ? "" : (run.StdOut ?? "");
                if (output.Length == 0) return ips;
                foreach (string raw in output.Split('\n'))
                {
                    string line = raw.Trim();
                    if (line.Length == 0 || !line.StartsWith("TCP", StringComparison.OrdinalIgnoreCase)) continue;
                    string[] parts = Regex.Split(line, "\\s+");
                    if (parts.Length < 5) continue;
                    int connPid;
                    if (!int.TryParse(parts[parts.Length - 1], out connPid) || connPid != pid) continue;
                    string remote = parts[2];
                    int colon = remote.LastIndexOf(':');
                    if (colon <= 0) continue;
                    string ip = remote.Substring(0, colon).Trim('[', ']');
                    if (ip.Length == 0 || ip == "0.0.0.0" || ip.StartsWith("127.")) continue;
                    if (IsPrivate(ip)) continue;
                    if (!ips.Contains(ip)) ips.Add(ip);
                }
            }
            catch { }
            return ips;
        }

        /// <summary>
        /// Наблюдение за приложением: N секунд собираем новые домены из журнала DNS и его адреса.
        /// </summary>
        public static List<string> Watch(int pid, int seconds, Action<int, string> progress, out List<string> ips)
        {
            ips = new List<string>();
            List<string> hosts = new List<string>();
            Dictionary<string, bool> seen = new Dictionary<string, bool>(StringComparer.OrdinalIgnoreCase);
            bool oldAll = SiteGuard.AllowAllProcesses;
            SiteGuard.AllowAllProcesses = true;   // у игр свои сетевые службы: смотрим запросы всех процессов
            try
            {
                for (int step = 0; step <= seconds; step += 2)
                {
                    string err;
                    int scanned;
                    List<string> found = SiteGuard.NewBrowserHosts(out err, out scanned);
                    foreach (string h in found)
                    {
                        if (IsNoiseHost(h)) continue;
                        if (!seen.ContainsKey(h)) { seen[h] = true; hosts.Add(h); }
                    }
                    foreach (string ip in RemoteIps(pid))
                    {
                        if (!ips.Contains(ip)) ips.Add(ip);
                    }
                    if (progress != null) progress((int)(100.0 * step / Math.Max(1, seconds)),
                        string.Format("Наблюдаем: доменов {0}, адресов {1} ({2} с из {3})", hosts.Count, ips.Count, step, seconds));
                    System.Threading.Thread.Sleep(2000);
                }
            }
            finally
            {
                SiteGuard.AllowAllProcesses = oldAll;
            }
            return hosts;
        }

        /// <summary>Проверяет домены и возвращает те, что похожи на блокировку ТСПУ.</summary>
        public static List<string> BlockedHosts(IEnumerable<string> hosts, Action<int, string> progress, out List<string> ok)
        {
            List<string> blocked = new List<string>();
            ok = new List<string>();
            List<string> all = new List<string>(hosts);
            for (int i = 0; i < all.Count; i++)
            {
                string host = all[i];
                if (progress != null) progress((int)(100.0 * i / Math.Max(1, all.Count)), "Проверяем " + host + " (" + (i + 1) + " из " + all.Count + ")");
                string reason;
                if (SiteDoctor.LooksBlockedPublic(host, out reason)) blocked.Add(host);
                else ok.Add(host);
            }
            return blocked;
        }

        private static bool IsNoiseHost(string host)
        {
            foreach (string n in Noise)
            {
                if (host == n || host.EndsWith("." + n, StringComparison.OrdinalIgnoreCase)) return true;
            }
            return false;
        }

        private static bool IsPrivate(string ip)
        {
            System.Net.IPAddress addr;
            if (!System.Net.IPAddress.TryParse(ip, out addr)) return false;
            byte[] b = addr.GetAddressBytes();
            if (b.Length != 4) return false;
            if (b[0] == 10) return true;
            if (b[0] == 172 && b[1] >= 16 && b[1] <= 31) return true;
            if (b[0] == 192 && b[1] == 168) return true;
            if (b[0] == 169 && b[1] == 254) return true;
            return false;
        }
    }
}
