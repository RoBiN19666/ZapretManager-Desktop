using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.IO;
using System.Management;
using System.Net;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.ServiceProcess;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using Microsoft.Win32;

namespace ZapretManager
{
    public enum LogLevel { Info, Ok, Warn, Error }

    /// <summary>Разделяемый журнал событий приложения.</summary>
    public static class Log
    {
        public static event Action<LogLevel, string> Appended;
        private static readonly object _lock = new object();
        private static string _file;

        public static string FilePath
        {
            get { return _file; }
            set { _file = value; }
        }

        public static void Write(LogLevel level, string message)
        {
            string line = string.Format("{0:HH:mm:ss}  {1}", DateTime.Now, message);
            lock (_lock)
            {
                if (!string.IsNullOrEmpty(_file))
                {
                    try
                    {
                        FileInfo fi = new FileInfo(_file);
                        if (fi.Exists && fi.Length > 512 * 1024) File.Delete(_file);
                        File.AppendAllText(_file, string.Format("{0:yyyy-MM-dd} {1}{2}", DateTime.Now, line, Environment.NewLine), Encoding.UTF8);
                    }
                    catch { }
                }
            }
            Action<LogLevel, string> h = Appended;
            if (h != null)
            {
                try { h(level, line); } catch { }
            }
        }

        public static void Info(string m) { Write(LogLevel.Info, m); }
        public static void Ok(string m) { Write(LogLevel.Ok, m); }
        public static void Warn(string m) { Write(LogLevel.Warn, m); }
        public static void Error(string m) { Write(LogLevel.Error, m); }
        public static void Error(string m, Exception ex) { Write(LogLevel.Error, m + ": " + (ex == null ? "" : ex.Message)); }
    }

    public static class Native
    {
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
        [DllImport("user32.dll")] public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
        [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
        [DllImport("dwmapi.dll")] private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attr, ref int attrValue, int attrSize);
        [DllImport("uxtheme.dll", CharSet = CharSet.Unicode)] private static extern int SetWindowTheme(IntPtr hWnd, string pszSubAppName, string pszSubIdList);

        /// <summary>Тёмные ползунки прокрутки у RichTextBox/ListBox (Windows 10+).</summary>
        public static void ApplyDarkScrollbars(IntPtr hwnd)
        {
            try { SetWindowTheme(hwnd, "DarkMode_Explorer", null); }
            catch { }
        }

        [DllImport("user32.dll", CharSet = CharSet.Auto)]
        private static extern IntPtr SendMessage(IntPtr hWnd, int msg, IntPtr wParam, IntPtr lParam);

        /// <summary>Перерисовка RichTextBox без мерцания (WM_SETREDRAW).</summary>
        public static void SetRedraw(IntPtr hwnd, bool enabled)
        {
            try { SendMessage(hwnd, 0x000B, (IntPtr)(enabled ? 1 : 0), IntPtr.Zero); } catch { }
        }

        public static bool IsAdmin()
        {
            try
            {
                using (WindowsIdentity id = WindowsIdentity.GetCurrent())
                {
                    WindowsPrincipal p = new WindowsPrincipal(id);
                    return p.IsInRole(WindowsBuiltInRole.Administrator);
                }
            }
            catch { return false; }
        }

        public static void EnableDarkTitleBar(IntPtr hwnd)
        {
            try
            {
                int v = 1;
                int hr = DwmSetWindowAttribute(hwnd, 20, ref v, 4);
                if (hr != 0) DwmSetWindowAttribute(hwnd, 19, ref v, 4);
            }
            catch { }
        }

        public static void OpenFolder(string path)
        {
            try
            {
                if (Directory.Exists(path)) Process.Start("explorer.exe", "\"" + path + "\"");
            }
            catch (Exception ex) { Log.Error("Не удалось открыть папку", ex); }
        }

        public static void SelectInExplorer(string file)
        {
            try
            {
                if (File.Exists(file)) Process.Start("explorer.exe", "/select,\"" + file + "\"");
                else OpenFolder(Path.GetDirectoryName(file));
            }
            catch (Exception ex) { Log.Error("Не удалось открыть проводник", ex); }
        }

        public static void OpenUrl(string url)
        {
            try { Process.Start(new ProcessStartInfo(url) { UseShellExecute = true }); }
            catch (Exception ex) { Log.Error("Не удалось открыть ссылку", ex); }
        }
    }

    public sealed class ProcResult
    {
        public int ExitCode;
        public string StdOut = "";
        public string StdErr = "";
        public bool TimedOut;

        public string All
        {
            get { return (StdOut + " " + StdErr).Trim(); }
        }
    }

    public static class Proc
    {
        public static ProcResult Run(string file, string args, int timeoutMs)
        {
            ProcResult r = new ProcResult();
            r.ExitCode = -1;
            try
            {
                ProcessStartInfo psi = new ProcessStartInfo(file, args);
                psi.UseShellExecute = false;
                psi.CreateNoWindow = true;
                psi.RedirectStandardOutput = true;
                psi.RedirectStandardError = true;
                try { psi.StandardOutputEncoding = Encoding.GetEncoding(866); psi.StandardErrorEncoding = Encoding.GetEncoding(866); } catch { }
                using (Process p = Process.Start(psi))
                {
                    r.StdOut = p.StandardOutput.ReadToEnd();
                    r.StdErr = p.StandardError.ReadToEnd();
                    if (!p.WaitForExit(timeoutMs)) { r.TimedOut = true; try { p.Kill(); } catch { } }
                    else r.ExitCode = p.ExitCode;
                }
            }
            catch (Exception ex)
            {
                r.StdErr = ex.Message;
            }
            return r;
        }

        /// <summary>Скрытый запуск процесса, возвращает объект процесса.</summary>
        public static Process StartHidden(string file, string args, string workDir)
        {
            ProcessStartInfo psi = new ProcessStartInfo(file, args);
            psi.UseShellExecute = false;
            psi.CreateNoWindow = true;
            psi.WindowStyle = ProcessWindowStyle.Hidden;
            if (!string.IsNullOrEmpty(workDir)) psi.WorkingDirectory = workDir;
            return Process.Start(psi);
        }

        private static readonly object _outLock = new object();
        private static readonly List<string> _recent = new List<string>();

        public static string RecentOutput
        {
            get { lock (_outLock) { return string.Join("\r\n", _recent.ToArray()); } }
        }

        public static void ClearOutput()
        {
            lock (_outLock) { _recent.Clear(); }
        }

        /// <summary>Запуск с перехватом stdout/stderr в общий буфер (для диагностики winws).</summary>
        public static Process StartCaptured(string file, string args, string workDir)
        {
            ProcessStartInfo psi = new ProcessStartInfo(file, args);
            psi.UseShellExecute = false;
            psi.CreateNoWindow = true;
            psi.WindowStyle = ProcessWindowStyle.Hidden;
            psi.RedirectStandardOutput = true;
            psi.RedirectStandardError = true;
            try { psi.StandardOutputEncoding = Encoding.GetEncoding(866); psi.StandardErrorEncoding = Encoding.GetEncoding(866); } catch { }
            if (!string.IsNullOrEmpty(workDir)) psi.WorkingDirectory = workDir;
            Process p = new Process();
            p.StartInfo = psi;
            p.OutputDataReceived += delegate(object s, DataReceivedEventArgs e) { AddOutput(e.Data); };
            p.ErrorDataReceived += delegate(object s, DataReceivedEventArgs e) { AddOutput(e.Data); };
            p.Start();
            try { p.BeginOutputReadLine(); p.BeginErrorReadLine(); } catch { }
            return p;
        }

        private static void AddOutput(string line)
        {
            if (string.IsNullOrEmpty(line)) return;
            lock (_outLock)
            {
                _recent.Add(line);
                while (_recent.Count > 40) _recent.RemoveAt(0);
            }
        }

        public sealed class ProcInfo
        {
            public int Pid;
            public string CommandLine = "";
            public DateTime? Started;
            public long WorkingSet;
        }

        /// <summary>Список процессов по имени exe-файла (с командной строкой).</summary>
        public static List<ProcInfo> List(string exeName)
        {
            List<ProcInfo> list = new List<ProcInfo>();
            try
            {
                string q = string.Format("SELECT ProcessId, CommandLine, CreationDate, WorkingSetSize FROM Win32_Process WHERE Name='{0}'", exeName.Replace("'", "''"));
                using (ManagementObjectSearcher s = new ManagementObjectSearcher(q))
                {
                    foreach (ManagementBaseObject o in s.Get())
                    {
                        ProcInfo pi = new ProcInfo();
                        try { pi.Pid = Convert.ToInt32(o["ProcessId"]); } catch { }
                        try { pi.CommandLine = Convert.ToString(o["CommandLine"]); } catch { }
                        try { pi.WorkingSet = Convert.ToInt64(o["WorkingSetSize"]); } catch { }
                        try
                        {
                            string cd = Convert.ToString(o["CreationDate"]);
                            if (!string.IsNullOrEmpty(cd)) pi.Started = ManagementDateTimeConverter.ToDateTime(cd);
                        }
                        catch { }
                        list.Add(pi);
                    }
                }
            }
            catch
            {
                // Резервный вариант без WMI
                try
                {
                    foreach (Process p in Process.GetProcessesByName(Path.GetFileNameWithoutExtension(exeName)))
                    {
                        ProcInfo pi = new ProcInfo();
                        pi.Pid = p.Id;
                        try { pi.Started = p.StartTime; } catch { }
                        list.Add(pi);
                        p.Dispose();
                    }
                }
                catch { }
            }
            return list;
        }

        /// <summary>Процессы, имя которых подходит под шаблон LIKE (например "TgWsProxy%").</summary>
        public static List<ProcInfo> ListLike(string namePattern)
        {
            List<ProcInfo> list = new List<ProcInfo>();
            try
            {
                string q = string.Format("SELECT ProcessId, Name, CommandLine, CreationDate, WorkingSetSize FROM Win32_Process WHERE Name LIKE '{0}'", namePattern.Replace("'", "''"));
                using (ManagementObjectSearcher s = new ManagementObjectSearcher(q))
                {
                    foreach (ManagementBaseObject o in s.Get())
                    {
                        ProcInfo pi = new ProcInfo();
                        try { pi.Pid = Convert.ToInt32(o["ProcessId"]); } catch { }
                        try { pi.CommandLine = Convert.ToString(o["CommandLine"]); } catch { }
                        try { pi.WorkingSet = Convert.ToInt64(o["WorkingSetSize"]); } catch { }
                        try
                        {
                            string cd = Convert.ToString(o["CreationDate"]);
                            if (!string.IsNullOrEmpty(cd)) pi.Started = ManagementDateTimeConverter.ToDateTime(cd);
                        }
                        catch { }
                        list.Add(pi);
                    }
                }
            }
            catch { }
            return list;
        }

        public static void KillAll(string exeName)
        {
            try
            {
                foreach (Process p in Process.GetProcessesByName(Path.GetFileNameWithoutExtension(exeName)))
                {
                    try { p.Kill(); p.WaitForExit(3000); } catch { }
                    p.Dispose();
                }
            }
            catch (Exception ex) { Log.Error("Не удалось завершить процесс", ex); }
        }

        public static bool AnyRunning(string exeName)
        {
            try
            {
                Process[] ps = Process.GetProcessesByName(Path.GetFileNameWithoutExtension(exeName));
                bool any = ps.Length > 0;
                foreach (Process p in ps) p.Dispose();
                return any;
            }
            catch { return false; }
        }
    }

    public static class Web
    {
        public const string UserAgent = "ZapretManager/1.0 (+https://github.com/RoBiN19666/zapret-discord-youtube)";

        static Web()
        {
            EnableModernTls();
            try { ServicePointManager.Expect100Continue = false; } catch { }
        }

        /// <summary>
        /// Явно включает TLS 1.2/1.3. Без этого .NET-процесс без настроек по умолчанию
        /// использует TLS 1.0, и сайты вроде Discord отклоняют рукопожатие.
        /// </summary>
        public static void EnableModernTls()
        {
            try
            {
                ServicePointManager.SecurityProtocol = SecurityProtocolType.Tls12 | (SecurityProtocolType)12288;
            }
            catch
            {
                try { ServicePointManager.SecurityProtocol = SecurityProtocolType.Tls12; } catch { }
            }
        }

        private static HttpWebRequest CreateRequest(string url, int timeoutMs)
        {
            HttpWebRequest req = (HttpWebRequest)WebRequest.Create(url);
            req.UserAgent = UserAgent;
            req.Timeout = timeoutMs;
            req.ReadWriteTimeout = timeoutMs;
            req.Proxy = null;
            req.AutomaticDecompression = DecompressionMethods.GZip | DecompressionMethods.Deflate;
            req.Headers.Add("Cache-Control", "no-cache");
            return req;
        }

        public static string GetString(string url, int timeoutMs)
        {
            using (HttpWebResponse resp = (HttpWebResponse)CreateRequest(url, timeoutMs).GetResponse())
            using (Stream s = resp.GetResponseStream())
            using (StreamReader sr = new StreamReader(s, Encoding.UTF8, true))
            {
                return sr.ReadToEnd();
            }
        }

        /// <summary>Адрес из заголовка Location (без перехода по редиректу).</summary>
        public static string GetRedirectLocation(string url, int timeoutMs)
        {
            HttpWebRequest req = CreateRequest(url, timeoutMs);
            req.AllowAutoRedirect = false;
            req.Method = "GET";
            try
            {
                using (HttpWebResponse resp = (HttpWebResponse)req.GetResponse())
                {
                    return resp.Headers["Location"];
                }
            }
            catch (WebException ex)
            {
                HttpWebResponse resp = ex.Response as HttpWebResponse;
                if (resp != null) return resp.Headers["Location"];
                throw;
            }
        }

        public static long GetContentLength(string url, int timeoutMs)
        {
            HttpWebRequest req = CreateRequest(url, timeoutMs);
            req.Method = "HEAD";
            using (HttpWebResponse resp = (HttpWebResponse)req.GetResponse())
            {
                return resp.ContentLength;
            }
        }

        /// <summary>Скачивание файла с отчётом о прогрессе (0..100, -1 если размер неизвестен).</summary>
        public static void Download(string url, string targetPath, Action<int, long, long> progress, int timeoutMs)
        {
            using (HttpWebResponse resp = (HttpWebResponse)CreateRequest(url, timeoutMs).GetResponse())
            using (Stream input = resp.GetResponseStream())
            {
                long total = resp.ContentLength;
                string dir = Path.GetDirectoryName(targetPath);
                if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir)) Directory.CreateDirectory(dir);
                using (FileStream output = new FileStream(targetPath, FileMode.Create, FileAccess.Write, FileShare.None))
                {
                    byte[] buffer = new byte[81920];
                    long read = 0;
                    int lastPercent = -1;
                    int n;
                    while ((n = input.Read(buffer, 0, buffer.Length)) > 0)
                    {
                        output.Write(buffer, 0, n);
                        read += n;
                        int percent = total > 0 ? (int)(read * 100 / total) : -1;
                        if (progress != null && percent != lastPercent)
                        {
                            lastPercent = percent;
                            progress(percent, read, total);
                        }
                    }
                }
            }
        }
    }

    /// <summary>Настройки самого менеджера (manager.ini рядом с exe).</summary>
    public sealed class AppConfig
    {
        private readonly Dictionary<string, string> _values = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        public string Path;

        public static string DefaultPath()
        {
            string dir = AppPaths.DataDir;
            return System.IO.Path.Combine(dir, "manager.ini");
        }

        public static AppConfig Load(string path)
        {
            AppConfig c = new AppConfig();
            c.Path = path;
            try
            {
                if (File.Exists(path))
                {
                    foreach (string raw in File.ReadAllLines(path, Encoding.UTF8))
                    {
                        string line = raw.Trim();
                        if (line.Length == 0 || line.StartsWith("#") || line.StartsWith("[") || line.StartsWith(";")) continue;
                        int eq = line.IndexOf('=');
                        if (eq <= 0) continue;
                        c._values[line.Substring(0, eq).Trim()] = line.Substring(eq + 1).Trim();
                    }
                }
            }
            catch (Exception ex) { Log.Error("Не удалось прочитать manager.ini", ex); }
            return c;
        }

        public void Save()
        {
            try
            {
                string dir = System.IO.Path.GetDirectoryName(Path);
                if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir)) Directory.CreateDirectory(dir);
                StringBuilder sb = new StringBuilder();
                sb.AppendLine("# Zapret Manager — настройки. Файл можно редактировать вручную.");
                List<string> keys = new List<string>(_values.Keys);
                keys.Sort(StringComparer.OrdinalIgnoreCase);
                foreach (string k in keys) sb.AppendLine(k + "=" + _values[k]);
                File.WriteAllText(Path, sb.ToString(), new UTF8Encoding(true));
            }
            catch (Exception ex) { Log.Error("Не удалось сохранить manager.ini", ex); }
        }

        public string Get(string key, string def)
        {
            string v;
            if (_values.TryGetValue(key, out v) && v != null) return v;
            return def;
        }

        public void Set(string key, string value)
        {
            _values[key] = value == null ? "" : value;
        }

        public bool GetBool(string key, bool def)
        {
            string v = Get(key, def ? "1" : "0").Trim().ToLowerInvariant();
            return v == "1" || v == "true" || v == "yes" || v == "on";
        }

        public void SetBool(string key, bool value) { Set(key, value ? "1" : "0"); }

        public int GetInt(string key, int def)
        {
            int v;
            if (int.TryParse(Get(key, ""), out v)) return v;
            return def;
        }

        public void SetInt(string key, int value) { Set(key, value.ToString(CultureInfo.InvariantCulture)); }
    }

    public static class AppPaths
    {
        public static string ExeDir
        {
            get { return Path.GetDirectoryName(System.Reflection.Assembly.GetExecutingAssembly().Location); }
        }

        private static string _data;
        public static string DataDir
        {
            get
            {
                if (_data != null) return _data;
                string candidate = Path.Combine(ExeDir, "data");
                try
                {
                    if (!Directory.Exists(candidate)) Directory.CreateDirectory(candidate);
                    string probe = Path.Combine(candidate, ".write-test");
                    File.WriteAllText(probe, "ok");
                    File.Delete(probe);
                    _data = candidate;
                }
                catch
                {
                    _data = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "ZapretManager");
                    try { Directory.CreateDirectory(_data); } catch { }
                }
                return _data;
            }
        }

        public static string TempDir
        {
            get
            {
                string d = Path.Combine(Path.GetTempPath(), "ZapretManager");
                try { if (!Directory.Exists(d)) Directory.CreateDirectory(d); } catch { }
                return d;
            }
        }
    }

    /// <summary>Состояние игрового фильтра (utils\game_filter.enabled).</summary>
    public sealed class GameFilterState
    {
        public string Mode = "disabled"; // disabled | all | tcp | udp
        public string TcpRange = "1024-65535";
        public string UdpRange = "1024-65535";

        public string TcpToken { get { return (Mode == "all" || Mode == "tcp") ? TcpRange : "12"; } }
        public string UdpToken { get { return (Mode == "all" || Mode == "udp") ? UdpRange : "12"; } }

        public string ModeText
        {
            get
            {
                if (Mode == "all") return "включён (TCP и UDP)";
                if (Mode == "tcp") return "включён (только TCP)";
                if (Mode == "udp") return "включён (только UDP)";
                return "выключен";
            }
        }

        public static GameFilterState Load(string root)
        {
            GameFilterState st = new GameFilterState();
            try
            {
                string file = Path.Combine(root, Path.Combine("utils", "game_filter.enabled"));
                if (!File.Exists(file)) return st;
                foreach (string raw in File.ReadAllLines(file))
                {
                    string line = raw.Trim();
                    int eq = line.IndexOf('=');
                    if (eq <= 0) continue;
                    string key = line.Substring(0, eq).Trim().ToLowerInvariant();
                    string val = line.Substring(eq + 1).Trim();
                    if (key == "mode") { if (val.Length > 0) st.Mode = val.ToLowerInvariant(); }
                    else if (key == "all") st.Mode = "all";
                    else if (key == "tcp") { if (val.Length == 0) st.Mode = "tcp"; else st.TcpRange = val; }
                    else if (key == "udp") { if (val.Length == 0) st.Mode = "udp"; else st.UdpRange = val; }
                }
                if (!IsRangeValid(st.TcpRange)) st.TcpRange = "1024-65535";
                if (!IsRangeValid(st.UdpRange)) st.UdpRange = "1024-65535";
            }
            catch (Exception ex) { Log.Error("Не удалось прочитать настройки игрового фильтра", ex); }
            return st;
        }

        public void Save(string root)
        {
            string file = Path.Combine(root, Path.Combine("utils", "game_filter.enabled"));
            string dir = Path.GetDirectoryName(file);
            if (!Directory.Exists(dir)) Directory.CreateDirectory(dir);
            StringBuilder sb = new StringBuilder();
            sb.AppendLine("mode=" + Mode);
            sb.AppendLine("tcp=" + TcpRange);
            sb.AppendLine("udp=" + UdpRange);
            File.WriteAllText(file, sb.ToString(), new UTF8Encoding(false));
        }

        public static bool IsRangeValid(string text)
        {
            if (string.IsNullOrEmpty(text)) return false;
            string[] parts = text.Split(',');
            foreach (string raw in parts)
            {
                string p = raw.Trim();
                if (p.Length == 0) return false;
                Match m = Regex.Match(p, "^([1-9][0-9]*)(-([1-9][0-9]*))?$");
                if (!m.Success) return false;
                int a;
                if (!int.TryParse(m.Groups[1].Value, out a) || a > 65535) return false;
                if (m.Groups[3].Success)
                {
                    int b;
                    if (!int.TryParse(m.Groups[3].Value, out b) || b > 65535 || b < a) return false;
                }
            }
            return true;
        }
    }

    public sealed class StrategyDef
    {
        public string File;        // полный путь к .bat
        public string Name;        // "general (ALT3)"
        public string ShortName;   // "ALT3"
        public string Kind;        // base | alt | exp | simple | faketls | other
        public string Description;
        public bool Warning;
        public int Order;

        public string Tooltip
        {
            get { return Name + " — " + Description + (Warning ? " (не рекомендуется авторами)" : ""); }
        }
    }

    public static class AppInfo
    {
        public const string Version = "1.2.2";
        public const string Title = "Zapret Manager";

        /// <summary>Служебный режим снимка окна (используется при отладке и для отчётов).</summary>
        public static string ShotPath;
        public static string ShotPage;
        public static string ShotList;
        public static int ShotScroll;

        /// <summary>Запуск свёрнутым в трей (--tray / --autostart).</summary>
        public static bool TrayMode;
        /// <summary>Автозапуск при входе в Windows: тихий старт и запуск компонентов.</summary>
        public static bool AutostartMode;
        /// <summary>Пользователь явно выбрал «Выход» (не сворачивать в трей).</summary>
        public static bool ExitRequested;
        /// <summary>Событие «покажи окно» для повторного запуска приложения.</summary>
        public static EventWaitHandle ActivateEvent;
    }

    /// <summary>Обнаруженная установка zapret.</summary>
    public sealed class ZapretInstall
    {
        public string Root = "";
        public string Version = "";
        public string VersionSource = "";
        public List<StrategyDef> Strategies = new List<StrategyDef>();

        public string BinDir { get { return Path.Combine(Root, "bin"); } }
        public string ListsDir { get { return Path.Combine(Root, "lists"); } }
        public string UtilsDir { get { return Path.Combine(Root, "utils"); } }
        public string ServiceBat { get { return Path.Combine(Root, "service.bat"); } }
        public string WinwsExe { get { return Path.Combine(BinDir, "winws.exe"); } }
        public string GameFilterFile { get { return Path.Combine(UtilsDir, "game_filter.enabled"); } }
        public string CheckUpdatesFile { get { return Path.Combine(UtilsDir, "check_updates.enabled"); } }
        public string IpsetFile { get { return Path.Combine(ListsDir, "ipset-all.txt"); } }
        public string IpsetBackup { get { return Path.Combine(ListsDir, "ipset-all.txt.backup"); } }

        public bool IsValid
        {
            get { return Root.Length > 0 && File.Exists(ServiceBat) && File.Exists(WinwsExe); }
        }

        public int StrategyCount { get { return Strategies.Count; } }

        public static bool LooksValid(string dir)
        {
            if (string.IsNullOrEmpty(dir)) return false;
            try
            {
                return File.Exists(Path.Combine(dir, "service.bat")) && File.Exists(Path.Combine(Path.Combine(dir, "bin"), "winws.exe"));
            }
            catch { return false; }
        }

        public static ZapretInstall Detect(string preferred)
        {
            List<string> candidates = new List<string>();
            if (!string.IsNullOrEmpty(preferred)) candidates.Add(preferred);

            // 1. Папка рядом с exe — основной сценарий: exe кладут внутрь папки zapret.
            string exeDir = AppPaths.ExeDir;
            candidates.Add(exeDir);
            try
            {
                foreach (string d in Directory.GetDirectories(exeDir))
                {
                    candidates.Add(d);
                    try
                    {
                        foreach (string d2 in Directory.GetDirectories(d)) candidates.Add(d2);
                    }
                    catch { }
                }
            }
            catch { }

            // 2. Папка, из которой установлена действующая служба zapret.
            string fromService = ServiceRootFromRegistry();
            if (!string.IsNullOrEmpty(fromService)) candidates.Add(fromService);

            try
            {
                DirectoryInfo parent = Directory.GetParent(exeDir);
                if (parent != null) candidates.Add(parent.FullName);
            }
            catch { }
            candidates.Add(@"C:\zapret");
            candidates.Add(@"C:\Zapret");

            foreach (string c in candidates)
            {
                if (LooksValid(c))
                {
                    ZapretInstall inst = new ZapretInstall();
                    inst.Root = Path.GetFullPath(c).TrimEnd('\\');
                    inst.Rescan();
                    return inst;
                }
            }
            ZapretInstall empty = new ZapretInstall();
            empty.Root = exeDir;
            return empty;
        }

        /// <summary>Папка установки, из которой запущена служба zapret (если служба есть).</summary>
        public static string ServiceRootFromRegistry()
        {
            try
            {
                using (RegistryKey k = Registry.LocalMachine.OpenSubKey(@"System\CurrentControlSet\Services\zapret"))
                {
                    if (k == null) return null;
                    string img = Convert.ToString(k.GetValue("ImagePath"));
                    if (string.IsNullOrEmpty(img)) return null;
                    img = img.Trim();
                    if (img.StartsWith("\""))
                    {
                        int end = img.IndexOf('"', 1);
                        if (end > 1) img = img.Substring(1, end - 1);
                    }
                    else
                    {
                        int sp = img.IndexOf(".exe", StringComparison.OrdinalIgnoreCase);
                        if (sp > 0) img = img.Substring(0, sp + 4);
                    }
                    string bin = Path.GetDirectoryName(img);
                    if (string.IsNullOrEmpty(bin)) return null;
                    DirectoryInfo root = Directory.GetParent(bin);
                    if (root == null) return null;
                    return root.FullName;
                }
            }
            catch { return null; }
        }

        public void Rescan()
        {
            Strategies = new List<StrategyDef>();
            Version = ReadVersion();
            try
            {
                string[] files = Directory.GetFiles(Root, "*.bat");
                List<string> names = new List<string>();
                foreach (string f in files)
                {
                    string n = Path.GetFileNameWithoutExtension(f);
                    if (n.StartsWith("service", StringComparison.OrdinalIgnoreCase)) continue;
                    if (MustIgnoreBat(n)) continue;
                    names.Add(n);
                }
                names.Sort(NaturalCompare);
                int order = 0;
                foreach (string n in names)
                {
                    StrategyDef s = Describe(n);
                    s.File = Path.Combine(Root, n + ".bat");
                    s.Order = order++;
                    Strategies.Add(s);
                }
            }
            catch (Exception ex) { Log.Error("Не удалось прочитать список стратегий", ex); }
        }

        private static bool MustIgnoreBat(string name)
        {
            string n = name.ToLowerInvariant();
            return n.Contains("update") || n.Contains("install") || n.Contains("uninstall") || n == "start" || n == "stop";
        }

        public static int NaturalCompare(string a, string b)
        {
            return NaturalKey(a).CompareTo(NaturalKey(b));
        }

        private static string NaturalKey(string s)
        {
            StringBuilder sb = new StringBuilder();
            int i = 0;
            while (i < s.Length)
            {
                char c = s[i];
                if (char.IsDigit(c))
                {
                    int j = i;
                    while (j < s.Length && char.IsDigit(s[j])) j++;
                    sb.Append(int.Parse(s.Substring(i, j - i)).ToString("D6"));
                    i = j;
                }
                else
                {
                    sb.Append(char.ToLowerInvariant(c));
                    i++;
                }
            }
            return sb.ToString();
        }

        private StrategyDef Describe(string name)
        {
            StrategyDef s = new StrategyDef();
            s.Name = name;
            string lower = name.ToLowerInvariant();
            bool isAlt = lower.Contains("alt");

            if (lower == "general")
            {
                s.Kind = "base";
                s.Description = "Базовая стратегия. Работает в большинстве случаев — начинайте с неё.";
            }
            else if (lower.Contains("simple fake"))
            {
                s.Kind = "simple";
                s.Description = "Упрощённая подмена фейк-пакетов. Хорошо работает на части провайдеров.";
            }
            else if (lower.Contains("fake tls auto"))
            {
                s.Kind = "faketls";
                s.Description = "Автоматический подбор фейк-TLS. Универсальный вариант, если базовая не помогла.";
            }
            else if (lower.Contains("exp"))
            {
                s.Kind = "exp";
                s.Description = "Экспериментальная стратегия. Возможны нестабильность и побочные эффекты.";
            }
            else if (isAlt)
            {
                s.Kind = "alt";
                s.Description = "Альтернативная стратегия. Пробуйте по порядку, если базовая не помогает.";
            }
            else
            {
                s.Kind = "other";
                s.Description = "Пользовательская или неизвестная стратегия.";
            }

            Match m = Regex.Match(name, @"(?:ALT|alt)\s*(\d+)?");
            string shortName = name;
            if (lower.StartsWith("general"))
            {
                string trimmed = name.Substring("general".Length).Trim();
                if (trimmed.StartsWith("(") && trimmed.EndsWith(")")) trimmed = trimmed.Substring(1, trimmed.Length - 2).Trim();
                if (trimmed.Length == 0) shortName = "BASE";
                else shortName = trimmed;
            }
            else if (m.Success && m.Groups[1].Success) shortName = "ALT" + m.Groups[1].Value;
            s.ShortName = shortName;

            s.Warning = FileContainsComment(Path.Combine(Root, name + ".bat"), "NOT RECOMMENDED");
            return s;
        }

        private static bool FileContainsComment(string file, string token)
        {
            try
            {
                if (!File.Exists(file)) return false;
                using (StreamReader r = new StreamReader(file, Encoding.UTF8, true))
                {
                    for (int i = 0; i < 14; i++)
                    {
                        string line = r.ReadLine();
                        if (line == null) break;
                        if (line.IndexOf(token, StringComparison.OrdinalIgnoreCase) >= 0) return true;
                    }
                }
            }
            catch { }
            return false;
        }

        private string ReadVersion()
        {
            try
            {
                if (File.Exists(ServiceBat))
                {
                    string text = File.ReadAllText(ServiceBat, Encoding.UTF8);
                    Match m = Regex.Match(text, "LOCAL_VERSION=([0-9][0-9\\.]*)");
                    if (m.Success) { VersionSource = "service.bat"; return m.Groups[1].Value.Trim(); }
                }
            }
            catch { }
            try
            {
                Match m = Regex.Match(Root, @"zapret-discord-youtube-([0-9][0-9\.]*)", RegexOptions.IgnoreCase);
                if (m.Success) { VersionSource = "имя папки"; return m.Groups[1].Value.Trim(); }
            }
            catch { }
            VersionSource = "";
            return "?";
        }

        public StrategyDef FindStrategy(string name)
        {
            if (string.IsNullOrEmpty(name)) return null;
            foreach (StrategyDef s in Strategies)
            {
                if (string.Equals(s.Name, name, StringComparison.OrdinalIgnoreCase)) return s;
                if (string.Equals(s.Name + ".bat", name, StringComparison.OrdinalIgnoreCase)) return s;
            }
            return null;
        }
    }

    /// <summary>Разбор .bat-файла стратегии в аргументы командной строки winws.exe.</summary>
    public static class BatParser
    {
        public static string[] GetArgv(ZapretInstall inst, StrategyDef strategy, GameFilterState gf, out string error)
        {
            error = null;
            string path = strategy.File;
            string[] lines;
            try { lines = File.ReadAllLines(path, Encoding.UTF8); }
            catch (Exception ex) { error = "Не удалось прочитать файл стратегии: " + ex.Message; return null; }

            Dictionary<string, string> vars = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            vars["GameFilterTCP"] = gf.TcpToken;
            vars["GameFilterUDP"] = gf.UdpToken;
            vars["GameFilter"] = (gf.Mode == "all") ? gf.TcpRange : ((gf.Mode == "tcp") ? gf.TcpRange : ((gf.Mode == "udp") ? gf.UdpRange : "12"));

            foreach (string raw in lines)
            {
                Match m = Regex.Match(raw.Trim(), "^set\\s+\"?([A-Za-z0-9_~]+)\\s*=\\s*(.*?)\"?$", RegexOptions.IgnoreCase);
                if (m.Success) vars[m.Groups[1].Value] = m.Groups[2].Value.Trim();
            }

            int idx = -1;
            for (int i = 0; i < lines.Length; i++)
            {
                if (lines[i].IndexOf("winws.exe", StringComparison.OrdinalIgnoreCase) >= 0) { idx = i; break; }
            }
            if (idx < 0) { error = "В файле нет строки запуска winws.exe"; return null; }

            StringBuilder sb = new StringBuilder();
            for (int i = idx; i < lines.Length; i++)
            {
                string line = lines[i].Trim();
                if (i == idx)
                {
                    int p = line.IndexOf("winws.exe", StringComparison.OrdinalIgnoreCase);
                    line = line.Substring(p + "winws.exe".Length).TrimStart();
                    if (line.StartsWith("\"")) line = line.Substring(1);
                }
                bool cont = line.EndsWith("^");
                if (cont) line = line.Substring(0, line.Length - 1);
                sb.Append(line);
                sb.Append(' ');
                if (!cont) break;
            }

            string cmd = sb.ToString();
            string rootWithSlash = inst.Root.EndsWith("\\") ? inst.Root : inst.Root + "\\";
            // Переменные могут ссылаться друг на друга (LISTS=%~dp0lists\), поэтому раскрываем в несколько проходов.
            for (int pass = 0; pass < 5 && cmd.IndexOf('%') >= 0; pass++)
            {
                string before = cmd;
                cmd = cmd.Replace("%~dp0", rootWithSlash);
                cmd = Regex.Replace(cmd, "%([A-Za-z0-9_~]+)%", delegate(Match m)
                {
                    string v;
                    if (vars.TryGetValue(m.Groups[1].Value, out v)) return v;
                    return m.Value;
                });
                if (cmd == before) break;
            }

            string[] argv = Tokenize(cmd);
            if (argv.Length == 0) { error = "Не удалось разобрать аргументы стратегии"; return null; }
            if (!vars.ContainsKey("BIN"))
            {
                // На некоторых сборках переменная BIN задаётся иначе — не критично.
            }
            return argv;
        }

        private static string[] Tokenize(string cmd)
        {
            List<string> list = new List<string>();
            StringBuilder cur = new StringBuilder();
            bool inQuotes = false;
            bool has = false;
            foreach (char c in cmd)
            {
                if (c == '"')
                {
                    inQuotes = !inQuotes;
                    cur.Append(c);
                    has = true;
                }
                else if (!inQuotes && (c == ' ' || c == '\t' || c == '\r' || c == '\n'))
                {
                    if (has) { list.Add(cur.ToString()); cur.Length = 0; has = false; }
                }
                else
                {
                    cur.Append(c);
                    has = true;
                }
            }
            if (has) list.Add(cur.ToString());
            return list.ToArray();
        }

        public static string Quote(string token)
        {
            if (token == null) return "\"\"";
            if (token.Length == 0) return "\"\"";
            if (token.IndexOf('"') >= 0) return token;   // уже содержит собственную кавычку вида --key="value"
            if (token.IndexOf(' ') < 0 && token.IndexOf('\t') < 0) return token;
            return "\"" + token + "\"";
        }

        public static string BuildCommandLine(string[] argv)
        {
            StringBuilder sb = new StringBuilder();
            foreach (string a in argv)
            {
                if (sb.Length > 0) sb.Append(' ');
                sb.Append(Quote(a));
            }
            return sb.ToString();
        }
    }

    public sealed class ServiceSnapshot
    {
        public string Name = "";
        public bool Installed;
        public string Status = "не установлена";
        public string Strategy = "";
        public string ImagePath = "";
    }

    public sealed class RuntimeState
    {
        public bool WinwsRunning;
        public int WinwsPid;
        public string WinwsCommandLine = "";
        public DateTime? WinwsStarted;
        public string MatchedStrategy = "";
        public bool ServiceInstalled;
        public string ServiceStatus = "не установлена";
        public string ServiceStrategy = "";
        public GameFilterState GameFilter;
        public string IpsetMode = "loaded";
        public long IpsetLines;
        public bool CheckUpdates;
    }

    /// <summary>Управление установкой zapret: запуск, служба, списки, ipset, hosts и т.д.</summary>
    public sealed class ZapretOps
    {
        private static bool _tcpChecked;
        public ZapretInstall Install;

        public ZapretOps(ZapretInstall install) { Install = install; }

        public static ServiceSnapshot QueryService(string name)
        {
            ServiceSnapshot snap = new ServiceSnapshot();
            snap.Name = name;
            try
            {
                using (ServiceController sc = new ServiceController(name))
                {
                    ServiceControllerStatus st = sc.Status;
                    snap.Installed = true;
                    snap.Status = TranslateStatus(st);
                }
            }
            catch (InvalidOperationException)
            {
                snap.Installed = false;
                snap.Status = "не установлена";
            }
            catch (Exception)
            {
                snap.Installed = false;
                snap.Status = "не установлена";
            }
            try
            {
                using (RegistryKey k = Registry.LocalMachine.OpenSubKey(@"System\CurrentControlSet\Services\" + name))
                {
                    if (k != null)
                    {
                        snap.Installed = true;
                        snap.ImagePath = Convert.ToString(k.GetValue("ImagePath"));
                        if (name == "zapret") snap.Strategy = Convert.ToString(k.GetValue("zapret-discord-youtube"));
                    }
                }
            }
            catch { }
            return snap;
        }

        private static string TranslateStatus(ServiceControllerStatus st)
        {
            switch (st)
            {
                case ServiceControllerStatus.Running: return "работает";
                case ServiceControllerStatus.Stopped: return "остановлена";
                case ServiceControllerStatus.StartPending: return "запускается";
                case ServiceControllerStatus.StopPending: return "останавливается";
                case ServiceControllerStatus.Paused: return "приостановлена";
                case ServiceControllerStatus.ContinuePending: return "возобновление";
                default: return st.ToString();
            }
        }

        public string ServiceInstallRoot
        {
            get { return ZapretInstall.ServiceRootFromRegistry(); }
        }

        /// <summary>Служба zapret установлена из другой папки, чем та, которой управляет менеджер.</summary>
        public bool ServicePointsElsewhere
        {
            get
            {
                try
                {
                    string sr = ServiceInstallRoot;
                    if (string.IsNullOrEmpty(sr) || string.IsNullOrEmpty(Install.Root)) return false;
                    if (!ZapretInstall.LooksValid(sr)) return false;
                    return !string.Equals(Path.GetFullPath(sr).TrimEnd('\\'), Path.GetFullPath(Install.Root).TrimEnd('\\'), StringComparison.OrdinalIgnoreCase);
                }
                catch { return false; }
            }
        }

        /// <summary>Служба установлена, но папка, из которой она запускается, больше не существует.</summary>
        public bool ServicePathMissing
        {
            get
            {
                try
                {
                    ServiceSnapshot svc = QueryService("zapret");
                    if (!svc.Installed) return false;
                    string root = ServiceInstallRoot;
                    return string.IsNullOrEmpty(root) || !ZapretInstall.LooksValid(root);
                }
                catch { return false; }
            }
        }

        public RuntimeState GetState()
        {
            RuntimeState st = new RuntimeState();
            st.GameFilter = GameFilterState.Load(Install.Root);
            st.CheckUpdates = File.Exists(Install.CheckUpdatesFile);
            st.IpsetMode = GetIpsetMode();
            st.IpsetLines = CountListLines(Install.IpsetFile);

            ServiceSnapshot svc = QueryService("zapret");
            st.ServiceInstalled = svc.Installed;
            st.ServiceStatus = svc.Status;
            st.ServiceStrategy = svc.Strategy;

            List<Proc.ProcInfo> procs = Proc.List("winws.exe");
            if (procs.Count > 0)
            {
                st.WinwsRunning = true;
                st.WinwsPid = procs[0].Pid;
                st.WinwsCommandLine = procs[0].CommandLine;
                st.WinwsStarted = procs[0].Started;
                st.MatchedStrategy = MatchStrategy(st.WinwsCommandLine);
            }
            // Стратегия из реестра службы — надёжный источник, если процесс запущен службой
            if (st.MatchedStrategy.Length == 0 && svc.Installed && svc.Status == "работает" && svc.Strategy.Length > 0)
            {
                st.MatchedStrategy = svc.Strategy;
            }
            return st;
        }

        public string MatchStrategy(string commandLine)
        {
            if (string.IsNullOrEmpty(commandLine)) return "";
            string norm = Normalize(commandLine);
            foreach (StrategyDef s in Install.Strategies)
            {
                try
                {
                    string err;
                    GameFilterState gf = GameFilterState.Load(Install.Root);
                    string[] argv = BatParser.GetArgv(Install, s, gf, out err);
                    if (argv == null) continue;
                    if (norm.EndsWith(Normalize(BatParser.BuildCommandLine(argv)))) return s.Name;
                }
                catch { }
            }
            return "";
        }

        private static string Normalize(string s)
        {
            if (s == null) return "";
            s = s.Replace("\"", " ").Replace("/", "\\");
            s = Regex.Replace(s, "\\s+", " ");
            return s.Trim().ToLowerInvariant();
        }

        public long CountListLines(string file)
        {
            try
            {
                if (!File.Exists(file)) return 0;
                long n = 0;
                using (StreamReader r = new StreamReader(file, Encoding.UTF8, true))
                {
                    while (r.ReadLine() != null) n++;
                }
                return n;
            }
            catch { return 0; }
        }

        // ---------- Запуск / остановка ----------

        public sealed class StartResult
        {
            public bool Ok;
            public string Message = "";
            public int Pid;
        }

        public StartResult Start(StrategyDef strategy)
        {
            StartResult res = new StartResult();
            if (!Native.IsAdmin())
            {
                res.Message = "Запуск требует прав администратора.";
                return res;
            }
            ServiceSnapshot svc = QueryService("zapret");
            if (svc.Installed && svc.Status == "работает")
            {
                res.Message = "Служба zapret уже запущена. Сначала удалите службу, чтобы запускать стратегии вручную.";
                return res;
            }
            if (Proc.AnyRunning("winws.exe") || Proc.List("winws.exe").Count > 0)
            {
                Log.Info("Останавливаю текущий winws.exe перед запуском новой стратегии…");
                Proc.KillAll("winws.exe");
                Thread.Sleep(400);
            }

            GameFilterState gf = GameFilterState.Load(Install.Root);
            string err;
            string[] argv = BatParser.GetArgv(Install, strategy, gf, out err);
            if (argv == null)
            {
                res.Message = err;
                return res;
            }
            string cmd = BatParser.BuildCommandLine(argv);
            try
            {
                if (!_tcpChecked) { EnableTcpTimestamps(); _tcpChecked = true; }
                Proc.ClearOutput();
                Process p = Proc.StartCaptured(Install.WinwsExe, cmd, Install.BinDir);
                Thread.Sleep(1200);
                if (p == null)
                {
                    res.Message = "Процесс winws.exe не создан.";
                    return res;
                }
                try
                {
                    if (p.HasExited)
                    {
                        string output = Proc.RecentOutput;
                        res.Message = string.Format("winws.exe завершился сразу после запуска (код {0}).{1}",
                            p.ExitCode, output.Length > 0 ? " Вывод: " + Shorten(output, 400) : " Проверьте права администратора и файл WinDivert64.sys.");
                        Log.Error(res.Message);
                        return res;
                    }
                }
                catch { }
                string early = Proc.RecentOutput;
                if (early.IndexOf("error", StringComparison.OrdinalIgnoreCase) >= 0 || early.IndexOf("failed", StringComparison.OrdinalIgnoreCase) >= 0)
                {
                    Log.Warn("winws.exe сообщает: " + Shorten(early, 400));
                }
                res.Ok = true;
                res.Pid = p.Id;
                res.Message = string.Format("Стратегия «{0}» запущена (PID {1}).", strategy.Name, p.Id);
                Log.Ok(res.Message);
                return res;
            }
            catch (Exception ex)
            {
                res.Message = "Ошибка запуска: " + ex.Message;
                Log.Error(res.Message);
                return res;
            }
        }

        private static string Shorten(string text, int max)
        {
            if (text == null) return "";
            text = text.Replace("\r", " ").Replace("\n", " ").Trim();
            return text.Length <= max ? text : text.Substring(0, max) + "…";
        }

        public string Stop()
        {
            ServiceSnapshot svc = QueryService("zapret");
            string msg = "";
            if (svc.Installed && svc.Status == "работает")
            {
                Proc.Run("sc.exe", "stop zapret", 20000);
                msg += "Служба zapret остановлена. ";
                Log.Info("Служба zapret остановлена.");
            }
            if (Proc.List("winws.exe").Count > 0)
            {
                Proc.KillAll("winws.exe");
                msg += "Процесс winws.exe завершён.";
                Log.Ok("Процесс winws.exe завершён.");
            }
            if (msg.Length == 0) msg = "Bypass уже остановлен.";
            return msg.Trim();
        }

        public string Restart(StrategyDef strategy)
        {
            Stop();
            Thread.Sleep(500);
            StartResult r = Start(strategy);
            return r.Message;
        }

        public bool EnableTcpTimestamps()
        {
            try
            {
                // chcp 437 — иначе netsh печатает подписи на языке системы и проверить состояние нельзя
                ProcResult r = Proc.Run("cmd.exe", "/c chcp 437 >nul & netsh interface tcp show global", 10000);
                string outText = r.All.ToLowerInvariant();
                int idx = outText.IndexOf("timestamps", StringComparison.Ordinal);
                if (idx >= 0)
                {
                    int colon = outText.IndexOf(':', idx);
                    if (colon > 0)
                    {
                        string value = outText.Substring(colon + 1).Trim();
                        if (value.StartsWith("enabled")) return true;
                    }
                }
                ProcResult set = Proc.Run("cmd.exe", "/c chcp 437 >nul & netsh interface tcp set global timestamps=enabled", 10000);
                if (set.ExitCode != 0)
                {
                    Log.Warn("netsh timestamps=enabled: " + set.All);
                    return false;
                }
                return true;
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось включить TCP timestamps", ex);
                return false;
            }
        }

        /// <summary>Состояние TCP timestamps: true — включены, false — выключены, null — не удалось определить.</summary>
        public static bool? TcpTimestampsEnabled()
        {
            try
            {
                ProcResult r = Proc.Run("cmd.exe", "/c chcp 437 >nul & netsh interface tcp show global", 10000);
                string text = r.All.ToLowerInvariant();
                int idx = text.IndexOf("timestamps", StringComparison.Ordinal);
                if (idx < 0) return null;
                int colon = text.IndexOf(':', idx);
                if (colon < 0) return null;
                string value = text.Substring(colon + 1).Trim();
                return value.StartsWith("enabled");
            }
            catch { return null; }
        }

        // ---------- Служба ----------

        public string InstallService(StrategyDef strategy, out bool ok)
        {
            ok = false;
            if (!Native.IsAdmin()) return "Установка службы требует прав администратора.";
            if (!File.Exists(Install.WinwsExe)) return "Файл bin\\winws.exe не найден.";

            GameFilterState gf = GameFilterState.Load(Install.Root);
            string err;
            string[] argv = BatParser.GetArgv(Install, strategy, gf, out err);
            if (argv == null) return err;
            string args = BatParser.BuildCommandLine(argv);

            EnableTcpTimestamps();
            Proc.Run("sc.exe", "stop zapret", 20000);
            Thread.Sleep(400);
            Proc.Run("sc.exe", "delete zapret", 20000);
            Proc.KillAll("winws.exe");
            Thread.Sleep(300);

            string binPath = "\"" + Install.WinwsExe + "\" " + args;
            string scArgs = "create zapret binPath= \"" + binPath.Replace("\"", "\\\"") + "\" DisplayName= \"zapret\" start= auto";
            ProcResult cr = Proc.Run("sc.exe", scArgs, 20000);
            if (cr.ExitCode != 0)
            {
                return "sc create завершился с кодом " + cr.ExitCode + ": " + cr.All;
            }
            Proc.Run("sc.exe", "description zapret \"Zapret DPI bypass software\"", 10000);
            ProcResult sr = Proc.Run("sc.exe", "start zapret", 25000);

            try
            {
                using (RegistryKey k = Registry.LocalMachine.CreateSubKey(@"System\CurrentControlSet\Services\zapret"))
                {
                    if (k != null) k.SetValue("zapret-discord-youtube", strategy.Name, RegistryValueKind.String);
                }
            }
            catch (Exception ex) { Log.Warn("Не удалось записать имя стратегии в реестр: " + ex.Message); }

            Thread.Sleep(900);
            ServiceSnapshot snap = QueryService("zapret");
            ok = snap.Installed;
            if (ok)
            {
                Log.Ok("Служба zapret установлена: " + strategy.Name);
                return string.Format("Служба установлена со стратегией «{0}». Статус: {1}.", strategy.Name, snap.Status);
            }
            return "Служба создана, но не обнаружена. " + sr.All;
        }

        public string RemoveService()
        {
            if (!Native.IsAdmin()) return "Удаление службы требует прав администратора.";
            StringBuilder sb = new StringBuilder();
            ServiceSnapshot svc = QueryService("zapret");
            if (svc.Installed)
            {
                Proc.Run("sc.exe", "stop zapret", 20000);
                Thread.Sleep(300);
                ProcResult d = Proc.Run("sc.exe", "delete zapret", 20000);
                sb.Append(d.ExitCode == 0 ? "Служба zapret удалена. " : "Не удалось удалить службу zapret: " + d.All + " ");
                Log.Ok("Служба zapret удалена.");
            }
            Proc.KillAll("winws.exe");
            foreach (string name in new string[] { "WinDivert", "WinDivert14" })
            {
                ServiceSnapshot w = QueryService(name);
                if (w.Installed)
                {
                    Proc.Run("sc.exe", "stop " + name, 12000);
                    Proc.Run("sc.exe", "delete " + name, 12000);
                    sb.Append("Драйвер " + name + " остановлен и удалён. ");
                }
            }
            if (sb.Length == 0) sb.Append("Служба не была установлена.");
            return sb.ToString().Trim();
        }

        // ---------- IPSet ----------

        public string GetIpsetMode()
        {
            try
            {
                if (!File.Exists(Install.IpsetFile)) return "none";
                string text = File.ReadAllText(Install.IpsetFile);
                if (text.Trim().Length == 0) return "any";
                if (text.IndexOf("203.0.113.113/32", StringComparison.Ordinal) >= 0 && text.Length < 200) return "none";
                return "loaded";
            }
            catch { return "loaded"; }
        }

        public string SetIpsetMode(string mode)
        {
            try
            {
                string current = GetIpsetMode();
                if (current == mode) return "Режим уже установлен: " + IpsetModeText(mode);
                if (current == "loaded")
                {
                    if (File.Exists(Install.IpsetBackup)) File.Delete(Install.IpsetBackup);
                    File.Move(Install.IpsetFile, Install.IpsetBackup);
                }
                else if (current == "none")
                {
                    File.WriteAllText(Install.IpsetFile, "", new UTF8Encoding(false));
                }
                else if (current == "any")
                {
                    if (File.Exists(Install.IpsetBackup))
                    {
                        if (File.Exists(Install.IpsetFile)) File.Delete(Install.IpsetFile);
                        File.Move(Install.IpsetBackup, Install.IpsetFile);
                    }
                    else return "Нет резервной копии списка. Сначала обновите IP-лист.";
                }

                if (mode == "none")
                {
                    File.WriteAllText(Install.IpsetFile, "203.0.113.113/32\r\n", new UTF8Encoding(false));
                }
                else if (mode == "any")
                {
                    File.WriteAllText(Install.IpsetFile, "", new UTF8Encoding(false));
                }
                else if (mode == "loaded")
                {
                    if (File.Exists(Install.IpsetBackup))
                    {
                        if (File.Exists(Install.IpsetFile)) File.Delete(Install.IpsetFile);
                        File.Move(Install.IpsetBackup, Install.IpsetFile);
                    }
                }
                Log.Ok("IPSet режим: " + IpsetModeText(mode));
                return "Готово: " + IpsetModeText(mode);
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось переключить IPSet", ex);
                return "Ошибка: " + ex.Message;
            }
        }

        public static string IpsetModeText(string mode)
        {
            if (mode == "any") return "любые IP (фильтр по списку хостов)";
            if (mode == "none") return "без фильтра по IP";
            return "загружен полный список";
        }

        public string UpdateIpset(Action<int, long, long> progress)
        {
            string url = "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/refs/heads/main/.service/ipset-service.txt";
            string tmp = Path.Combine(AppPaths.TempDir, "ipset-all.txt");
            try
            {
                Log.Info("Скачиваю IP-лист…");
                Web.Download(url, tmp, progress, 60000);
                string text = File.ReadAllText(tmp);
                if (text.Trim().Length == 0) return "Скачанный список пуст — обновление отменено.";
                int lines = 0;
                foreach (string l in text.Split('\n')) if (l.Trim().Length > 0) lines++;
                if (lines < 100) return string.Format("Список подозрительно мал ({0} строк) — обновление отменено.", lines);

                string mode = GetIpsetMode();
                if (mode == "loaded")
                {
                    if (File.Exists(Install.IpsetBackup)) File.Delete(Install.IpsetBackup);
                    File.Copy(tmp, Install.IpsetFile, true);
                }
                else
                {
                    File.Copy(tmp, Install.IpsetBackup, true);
                    if (mode == "any") SetIpsetRestoreAfterUpdate();
                }
                long count = CountListLines(Install.IpsetFile);
                Log.Ok(string.Format("IP-лист обновлён: {0} строк.", count));
                return string.Format("IP-лист обновлён: {0} строк.", count);
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось обновить IP-лист", ex);
                return "Ошибка обновления: " + ex.Message;
            }
        }

        private void SetIpsetRestoreAfterUpdate()
        {
            // в режиме "any" рабочий файл остаётся пустым, полный список лежит в .backup
            if (GetIpsetMode() != "any") File.WriteAllText(Install.IpsetFile, "", new UTF8Encoding(false));
        }

        // ---------- Фейки ----------

        public sealed class FakeFile
        {
            public string Name;
            public string FullPath;
            public string Sha256;
        }

        public List<FakeFile> ListFakes()
        {
            List<FakeFile> list = new List<FakeFile>();
            try
            {
                foreach (string f in Directory.GetFiles(Install.BinDir, "*.bin"))
                {
                    string n = Path.GetFileName(f);
                    if (n.StartsWith("ACTIVE_", StringComparison.OrdinalIgnoreCase)) continue;
                    FakeFile ff = new FakeFile();
                    ff.Name = Path.GetFileNameWithoutExtension(f);
                    ff.FullPath = f;
                    ff.Sha256 = Sha256File(f);
                    list.Add(ff);
                }
                list.Sort(delegate(FakeFile a, FakeFile b) { return ZapretInstall.NaturalCompare(a.Name, b.Name); });
            }
            catch (Exception ex) { Log.Error("Не удалось прочитать список фейков", ex); }
            return list;
        }

        public static string Sha256File(string path)
        {
            try
            {
                using (FileStream fs = File.OpenRead(path))
                using (System.Security.Cryptography.SHA256 sha = System.Security.Cryptography.SHA256.Create())
                {
                    byte[] hash = sha.ComputeHash(fs);
                    StringBuilder sb = new StringBuilder();
                    foreach (byte b in hash) sb.Append(b.ToString("x2"));
                    return sb.ToString();
                }
            }
            catch { return ""; }
        }

        public string FindActiveFake(string activeFileName)
        {
            string path = Path.Combine(Install.BinDir, activeFileName);
            if (!File.Exists(path)) return "(не найден)";
            string hash = Sha256File(path);
            foreach (FakeFile f in ListFakes())
            {
                if (string.Equals(f.Sha256, hash, StringComparison.OrdinalIgnoreCase)) return f.Name;
            }
            return "(пользовательский)";
        }

        public string ReplaceActiveFake(string activeFileName, string sourceBinPath)
        {
            try
            {
                string target = Path.Combine(Install.BinDir, activeFileName);
                File.Copy(sourceBinPath, target, true);
                string name = Path.GetFileNameWithoutExtension(sourceBinPath);
                Log.Ok("Активный фейк " + activeFileName + " заменён на " + name);
                return "Активный фейк заменён на «" + name + "». Перезапустите bypass, чтобы применить.";
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось заменить фейк", ex);
                return "Ошибка: " + ex.Message;
            }
        }

        // ---------- hosts ----------

        public const string HostsMarkerStart = "# >>> zapret-manager: начало блока";
        public const string HostsMarkerEnd = "# <<< zapret-manager: конец блока";

        public static string HostsPath
        {
            get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "drivers\\etc\\hosts"); }
        }

        public sealed class HostsStatus
        {
            public bool Downloaded;
            public int RemoteLines;
            public int MissingLines;
            public string Error = "";
            public string TempFile = "";
        }

        public HostsStatus CheckHosts()
        {
            HostsStatus st = new HostsStatus();
            string url = "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/refs/heads/main/.service/hosts";
            string tmp = Path.Combine(AppPaths.TempDir, "hosts-service.txt");
            try
            {
                Web.Download(url, tmp, null, 30000);
                st.Downloaded = true;
                st.TempFile = tmp;
                List<string> remote = ReadHostsHosts(tmp);
                st.RemoteLines = remote.Count;
                string current = File.Exists(HostsPath) ? File.ReadAllText(HostsPath, Encoding.UTF8) : "";
                foreach (string line in remote)
                {
                    if (current.IndexOf(line, StringComparison.OrdinalIgnoreCase) < 0) st.MissingLines++;
                }
            }
            catch (Exception ex)
            {
                st.Error = ex.Message;
            }
            return st;
        }

        private static List<string> ReadHostsHosts(string file)
        {
            List<string> list = new List<string>();
            foreach (string raw in File.ReadAllLines(file, Encoding.UTF8))
            {
                string l = raw.Trim();
                if (l.Length == 0 || l.StartsWith("#")) continue;
                list.Add(l);
            }
            return list;
        }

        public string ApplyHosts()
        {
            if (!Native.IsAdmin()) return "Изменение hosts требует прав администратора.";
            HostsStatus st = CheckHosts();
            if (!st.Downloaded) return "Не удалось скачать список hosts: " + st.Error;
            if (st.MissingLines == 0) return "Файл hosts уже содержит все записи из репозитория.";
            try
            {
                string content = File.Exists(HostsPath) ? File.ReadAllText(HostsPath, Encoding.UTF8) : "";
                int s = content.IndexOf(HostsMarkerStart, StringComparison.Ordinal);
                int e = content.IndexOf(HostsMarkerEnd, StringComparison.Ordinal);
                if (s >= 0 && e > s)
                {
                    int endOfLine = content.IndexOf('\n', e);
                    if (endOfLine < 0) endOfLine = content.Length;
                    content = content.Substring(0, s) + content.Substring(endOfLine + 1);
                }
                List<string> remote = ReadHostsHosts(st.TempFile);
                StringBuilder sb = new StringBuilder();
                sb.AppendLine(content.TrimEnd());
                sb.AppendLine();
                sb.AppendLine(HostsMarkerStart);
                foreach (string l in remote) sb.AppendLine(l);
                sb.AppendLine(HostsMarkerEnd);

                string backup = HostsPath + ".zapret-manager.bak";
                if (!File.Exists(backup) && File.Exists(HostsPath)) File.Copy(HostsPath, backup, true);
                File.WriteAllText(HostsPath, sb.ToString(), new UTF8Encoding(false));
                Log.Ok(string.Format("Файл hosts обновлён: добавлено {0} строк.", st.MissingLines));
                return string.Format("Файл hosts обновлён (добавлено {0} строк). Резервная копия: {1}", st.MissingLines, backup);
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось обновить hosts", ex);
                return "Ошибка: " + ex.Message;
            }
        }

        public string ClearHostsBlock()
        {
            if (!Native.IsAdmin()) return "Изменение hosts требует прав администратора.";
            try
            {
                if (!File.Exists(HostsPath)) return "Файл hosts не найден.";
                string content = File.ReadAllText(HostsPath, Encoding.UTF8);
                int s = content.IndexOf(HostsMarkerStart, StringComparison.Ordinal);
                if (s < 0) return "Блок zapret-manager в hosts не найден.";
                int e = content.IndexOf(HostsMarkerEnd, StringComparison.Ordinal);
                int endOfLine = e > 0 ? content.IndexOf('\n', e) : content.Length;
                if (endOfLine < 0) endOfLine = content.Length;
                string result = content.Substring(0, s) + content.Substring(endOfLine + 1);
                File.WriteAllText(HostsPath, result, new UTF8Encoding(false));
                Log.Ok("Блок zapret-manager удалён из hosts.");
                return "Блок zapret-manager удалён из hosts.";
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось очистить hosts", ex);
                return "Ошибка: " + ex.Message;
            }
        }

        // ---------- Кэш Discord ----------

        public static List<string> DiscordCacheDirs()
        {
            List<string> dirs = new List<string>();
            string appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
            string[] names = new string[] { "discord", "discordptb", "discordcanary", "discorddevelopment" };
            foreach (string n in names)
            {
                string d = Path.Combine(appData, n);
                if (Directory.Exists(d)) dirs.Add(d);
            }
            return dirs;
        }

        public string ClearDiscordCache()
        {
            StringBuilder sb = new StringBuilder();
            List<string> dirs = DiscordCacheDirs();
            if (dirs.Count == 0) return "Установки Discord не найдены.";
            foreach (string proc in new string[] { "Discord", "DiscordPTB", "DiscordCanary", "DiscordDevelopment" })
            {
                try
                {
                    foreach (Process p in Process.GetProcessesByName(proc))
                    {
                        try { p.Kill(); sb.Append("Закрыт " + proc + ". "); } catch { }
                        p.Dispose();
                    }
                }
                catch { }
            }
            foreach (string dir in dirs)
            {
                foreach (string sub in new string[] { "Cache", "Code Cache", "GPUCache" })
                {
                    string path = Path.Combine(dir, sub);
                    try
                    {
                        if (Directory.Exists(path))
                        {
                            Directory.Delete(path, true);
                            sb.Append("Очищено: " + path + ". ");
                        }
                    }
                    catch (Exception ex)
                    {
                        sb.Append("Не удалось удалить " + path + " (" + ex.Message + "). ");
                    }
                }
            }
            Log.Info("Кэш Discord очищен: " + sb);
            return sb.ToString().Trim();
        }
    }
}
