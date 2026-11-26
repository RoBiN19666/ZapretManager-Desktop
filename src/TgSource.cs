using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Compression;
using System.Text;
using System.Threading;
using System.Windows.Forms;

namespace ZapretManager
{
    /// <summary>
    /// Тихий режим TG WS Proxy: запуск официальных исходников Flowseal без трея.
    /// Нужен Python 3.9+ и venv с зависимостями; зато один процесс и нет иконки в трее.
    /// </summary>
    public static class TgSource
    {
        public const string Repo = "Flowseal/tg-ws-proxy";
        private static readonly string[] RequiredPackages = new string[]
        {
            "httpx[http2]==0.28.1",
            "certifi",
            "cryptography"
        };

        /// <summary>Последний тег релиза для исходников (или main, если тег не определить).</summary>
        public static string LatestRef()
        {
            try
            {
                string tag = GitHub.LatestTagByRedirect(Repo);
                return string.IsNullOrEmpty(tag) ? "main" : tag;
            }
            catch { return "main"; }
        }

        public static string RootDir
        {
            get { return Path.Combine(Path.GetDirectoryName(TgProxy.DefaultDir), "tgwsproxy-src"); }
        }

        public static string SrcDir { get { return Path.Combine(RootDir, "src"); } }
        public static string VenvDir { get { return Path.Combine(RootDir, ".venv"); } }
        public static string VenvPython { get { return Path.Combine(VenvDir, @"Scripts\pythonw.exe"); } }
        public static string VenvConsolePython { get { return Path.Combine(VenvDir, @"Scripts\python.exe"); } }
        public static string MarkerPath { get { return Path.Combine(RootDir, "prepared.txt"); } }
        public static string ScriptPath { get { return Path.Combine(SrcDir, @"proxy\tg_ws_proxy.py"); } }

        public static bool IsPrepared
        {
            get
            {
                try
                {
                    return File.Exists(VenvPython) && File.Exists(ScriptPath) && File.Exists(MarkerPath);
                }
                catch { return false; }
            }
        }

        public static string PreparedInfo
        {
            get
            {
                try
                {
                    return File.Exists(MarkerPath) ? File.ReadAllText(MarkerPath, Encoding.UTF8).Trim() : "";
                }
                catch { return ""; }
            }
        }

        /// <summary>Ищет установленный Python 3 (PATH, типичные папки, лаунчер py).</summary>
        public static string FindPython()
        {
            List<string> candidates = new List<string>();
            try
            {
                string path = Environment.GetEnvironmentVariable("PATH") ?? "";
                foreach (string dir in path.Split(';'))
                {
                    if (dir.Trim().Length == 0) continue;
                    string exe = Path.Combine(dir.Trim(), "python.exe");
                    if (File.Exists(exe)) candidates.Add(exe);
                }
            }
            catch { }

            try
            {
                string local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
                string programs = Path.Combine(local, "Programs", "Python");
                if (Directory.Exists(programs))
                {
                    foreach (string dir in Directory.GetDirectories(programs))
                    {
                        string exe = Path.Combine(dir, "python.exe");
                        if (File.Exists(exe)) candidates.Add(exe);
                    }
                }
                string pf = Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles);
                if (Directory.Exists(pf))
                {
                    foreach (string dir in Directory.GetDirectories(pf, "Python*"))
                    {
                        string exe = Path.Combine(dir, "python.exe");
                        if (File.Exists(exe)) candidates.Add(exe);
                    }
                }
            }
            catch { }

            candidates.Sort(delegate(string a, string b) { return string.CompareOrdinal(b, a); });
            foreach (string exe in candidates)
            {
                string version = Run(exe, "--version", 8000);
                if (version.IndexOf("Python 3.", StringComparison.OrdinalIgnoreCase) >= 0) return exe;
            }
            return null;
        }

        private static string Run(string exe, string arguments, int timeoutMs)
        {
            try
            {
                ProcessStartInfo psi = new ProcessStartInfo(exe, arguments);
                psi.UseShellExecute = false;
                psi.CreateNoWindow = true;
                psi.RedirectStandardOutput = true;
                psi.RedirectStandardError = true;
                using (Process p = Process.Start(psi))
                {
                    string output = "";
                    Thread reader = new Thread(delegate()
                    {
                        try { output = p.StandardOutput.ReadToEnd() + p.StandardError.ReadToEnd(); }
                        catch { }
                    });
                    reader.IsBackground = true;
                    reader.Start();
                    if (!p.WaitForExit(timeoutMs))
                    {
                        try { p.Kill(); } catch { }
                        return output;
                    }
                    reader.Join(1500);
                    return output;
                }
            }
            catch (Exception ex)
            {
                return "ERR: " + ex.Message;
            }
        }

        /// <summary>Скачивает исходники, создаёт venv и ставит зависимости.</summary>
        public static string Prepare(string tag, Action<int, string> progress, out bool ok)
        {
            ok = false;
            try
            {
                string python = FindPython();
                if (python == null)
                    return "Python 3 не найден. Установите Python с python.org (галочка «Add python.exe to PATH») и повторите.";

                if (progress != null) progress(3, "Python: " + python);
                string refName = string.IsNullOrEmpty(tag) ? "main" : tag;
                string zip = Path.Combine(Path.GetTempPath(), "tgws-" + refName + ".zip");
                string url = string.Format("https://github.com/{0}/archive/refs/{1}.zip", Repo,
                    refName == "main" ? "heads/main" : "tags/" + refName);

                if (progress != null) progress(8, "Скачивание исходников " + refName + "…");
                Web.Download(url, zip, delegate(int percent, long read, long total)
                {
                    if (progress != null) progress(percent < 0 ? 15 : 8 + (int)(percent * 0.22), "Скачивание исходников: " + (percent < 0 ? 0 : percent) + "%");
                }, 180000);

                if (Directory.Exists(RootDir)) Directory.Delete(RootDir, true);
                Directory.CreateDirectory(RootDir);
                string tmp = Path.Combine(RootDir, "unpack");
                Directory.CreateDirectory(tmp);

                if (progress != null) progress(32, "Распаковка…");
                ZipFile.ExtractToDirectory(zip, tmp);
                string found = null;
                foreach (string dir in Directory.GetDirectories(tmp))
                {
                    if (File.Exists(Path.Combine(dir, @"proxy\tg_ws_proxy.py"))) { found = dir; break; }
                }
                if (found == null) return "В архиве не найден proxy/tg_ws_proxy.py — похоже, изменилась структура репозитория.";
                Directory.Move(found, SrcDir);
                try { Directory.Delete(tmp, true); } catch { }
                try { File.Delete(zip); } catch { }

                if (progress != null) progress(40, "Создаём виртуальное окружение…");
                string venvOut = Run(python, "-m venv \"" + VenvDir + "\"", 180000);
                if (!File.Exists(VenvConsolePython)) return "Не удалось создать venv: " + Trim(venvOut);

                if (progress != null) progress(60, "Устанавливаем зависимости (httpx, certifi, cryptography)…");
                StringBuilder args = new StringBuilder();
                args.Append("-m pip install --disable-pip-version-check --no-input ");
                foreach (string pkg in RequiredPackages) args.Append("\"").Append(pkg).Append("\" ");
                string pipOut = Run(VenvConsolePython, args.ToString(), 420000);
                if (pipOut.IndexOf("ERROR", StringComparison.OrdinalIgnoreCase) >= 0 || !File.Exists(VenvPython))
                    return "Не удалось установить зависимости: " + Trim(pipOut);

                if (progress != null) progress(92, "Проверяем запуск…");
                string check = Run(VenvConsolePython, "\"" + ScriptPath + "\" --help", 30000);
                if (check.IndexOf("usage", StringComparison.OrdinalIgnoreCase) < 0 && check.IndexOf("error", StringComparison.OrdinalIgnoreCase) >= 0)
                    return "Прокси не запускается из исходников: " + Trim(check);

                File.WriteAllText(MarkerPath,
                    "версия: " + refName + Environment.NewLine +
                    "python: " + python + Environment.NewLine +
                    "подготовлено: " + DateTime.Now.ToString("yyyy-MM-dd HH:mm") + Environment.NewLine,
                    new UTF8Encoding(false));

                if (progress != null) progress(100, "Готово");
                ok = true;
                Log.Ok("Тихий режим TG WS Proxy подготовлен (" + refName + ", " + python + ").");
                return "Готово: исходники " + refName + " и зависимости установлены в " + RootDir;
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось подготовить тихий режим TG WS Proxy", ex);
                return "Ошибка подготовки: " + ex.Message;
            }
        }

        private static string Trim(string text)
        {
            if (string.IsNullOrEmpty(text)) return "(без вывода)";
            text = text.Replace("\r", " ").Replace("\n", " ").Trim();
            return text.Length > 300 ? text.Substring(0, 299) + "…" : text;
        }

        public static List<Proc.ProcInfo> Find()
        {
            List<Proc.ProcInfo> result = new List<Proc.ProcInfo>();
            foreach (string exe in new string[] { "python.exe", "pythonw.exe" })
            {
                foreach (Proc.ProcInfo pi in Proc.List(exe))
                {
                    if (pi.CommandLine == null) continue;
                    if (pi.CommandLine.IndexOf("tg_ws_proxy.py", StringComparison.OrdinalIgnoreCase) < 0) continue;
                    if (pi.CommandLine.IndexOf("tgwsproxy-src", StringComparison.OrdinalIgnoreCase) < 0) continue;
                    result.Add(pi);
                }
            }
            return result;
        }

        public static bool Start(TgProxyState state, out int pid, out string error)
        {
            pid = 0;
            error = "";
            if (!IsPrepared)
            {
                error = "Тихий режим не подготовлен: нажмите «Подготовить» на странице «TG-прокси».";
                return false;
            }

            List<Proc.ProcInfo> running = Find();
            if (running.Count > 0)
            {
                pid = running[0].Pid;
                return true;
            }

            int port = state != null && state.Port > 0 ? state.Port : 1080;
            string secret = state != null ? (state.Secret ?? "") : "";
            string log = Path.Combine(Path.GetDirectoryName(TgProxy.DefaultDir), "tgwsproxy-source.log");

            StringBuilder args = new StringBuilder();
            args.Append("\"").Append(ScriptPath).Append("\"");
            args.Append(" --host 127.0.0.1 --port ").Append(port);
            if (secret.Length == 32) args.Append(" --secret ").Append(secret);
            args.Append(" --log-file \"").Append(log).Append("\"");

            try
            {
                // Запускаем системным pythonw, подсовывая пакеты из venv:
                // так получается один процесс, а не лаунчер venv + интерпретатор.
                string python = BasePython();
                string pythonw = python;
                string dir = Path.GetDirectoryName(python);
                string candidate = Path.Combine(dir, "pythonw.exe");
                if (File.Exists(candidate)) pythonw = candidate;
                if (!File.Exists(pythonw))
                {
                    pythonw = VenvPython;
                }

                ProcessStartInfo psi = new ProcessStartInfo(pythonw, args.ToString());
                psi.WorkingDirectory = SrcDir;
                psi.UseShellExecute = false;
                psi.CreateNoWindow = true;
                psi.WindowStyle = ProcessWindowStyle.Hidden;
                string sitePackages = Path.Combine(VenvDir, @"Lib\site-packages");
                if (Directory.Exists(sitePackages))
                {
                    string oldPath = Environment.GetEnvironmentVariable("PYTHONPATH") ?? "";
                    psi.EnvironmentVariables["PYTHONPATH"] = oldPath.Length > 0 ? sitePackages + ";" + oldPath : sitePackages;
                }
                Process p = Process.Start(psi);
                if (p != null) pid = p.Id;
                for (int i = 0; i < 25; i++)
                {
                    Thread.Sleep(300);
                    List<Proc.ProcInfo> list = Find();
                    if (list.Count > 0)
                    {
                        pid = list[0].Pid;
                        Log.Ok("TG WS Proxy запущен из исходников (PID " + pid + ", порт " + port + ").");
                        return true;
                    }
                }
                error = "Процесс не появился. Проверьте путь к Python и журнал " + log;
                return false;
            }
            catch (Exception ex)
            {
                error = ex.Message;
                Log.Error("Не удалось запустить тихий режим TG WS Proxy", ex);
                return false;
            }
        }

        public static int Stop()
        {
            int count = 0;
            foreach (Proc.ProcInfo pi in Find())
            {
                try
                {
                    using (Process p = Process.GetProcessById(pi.Pid))
                    {
                        p.Kill();
                        p.WaitForExit(3000);
                        count++;
                    }
                }
                catch { }
            }
            if (count > 0) Log.Info("Тихий режим TG WS Proxy остановлен (процессов: " + count + ").");
            return count;
        }

        /// <summary>Системный Python из маркера подготовки (или найденный заново).</summary>
        public static string BasePython()
        {
            string info = PreparedInfo;
            foreach (string line in info.Replace("\r\n", "\n").Split('\n'))
            {
                string t = line.Trim();
                if (t.StartsWith("python:", StringComparison.OrdinalIgnoreCase))
                {
                    string path = t.Substring(t.IndexOf(':') + 1).Trim();
                    if (File.Exists(path)) return path;
                }
            }
            string found = FindPython();
            return found ?? "";
        }

        public static string LogPath
        {
            get { return Path.Combine(Path.GetDirectoryName(TgProxy.DefaultDir), "tgwsproxy-source.log"); }
        }

        /// <summary>Версия из маркера подготовки: «v1.12.0» или «main».</summary>
        public static string PreparedVersion()
        {
            string info = PreparedInfo;
            foreach (string line in info.Replace("\r\n", "\n").Split('\n'))
            {
                string t = line.Trim();
                if (t.StartsWith("версия:", StringComparison.OrdinalIgnoreCase))
                {
                    string v = t.Substring(t.IndexOf(':') + 1).Trim();
                    return v.Length > 0 ? v : "исходники";
                }
            }
            return "исходники";
        }
    }
}
