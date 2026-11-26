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
        /// <summary>Портативный Python (embeddable-сборка python.org) — не требует установки у пользователя.</summary>
        public static string PortableDir { get { return Path.Combine(RootDir, "python"); } }
        public static string PortablePython { get { return Path.Combine(PortableDir, "pythonw.exe"); } }
        public static string PortableConsole { get { return Path.Combine(PortableDir, "python.exe"); } }
        public static string PortableSitePackages { get { return Path.Combine(PortableDir, @"Lib\site-packages"); } }
        public const string PythonVersion = "3.13.5";
        public static string MarkerPath { get { return Path.Combine(RootDir, "prepared.txt"); } }
        public static string ScriptPath { get { return Path.Combine(SrcDir, @"proxy\tg_ws_proxy.py"); } }

        public static bool IsPrepared
        {
            get
            {
                try
                {
                    return (File.Exists(PortablePython) || File.Exists(VenvPython)) && File.Exists(ScriptPath) && File.Exists(MarkerPath);
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
            return RunProcess(exe, arguments, timeoutMs, "", "");
        }

        /// <summary>Запуск процесса с асинхронным чтением обоих потоков (без дедлока на pip).</summary>
        private static string RunProcess(string exe, string arguments, int timeoutMs, string workingDir, string pythonPath)
        {
            try
            {
                ProcessStartInfo psi = new ProcessStartInfo(exe, arguments);
                psi.UseShellExecute = false;
                psi.CreateNoWindow = true;
                psi.RedirectStandardOutput = true;
                psi.RedirectStandardError = true;
                if (!string.IsNullOrEmpty(workingDir)) psi.WorkingDirectory = workingDir;
                if (!string.IsNullOrEmpty(pythonPath)) psi.EnvironmentVariables["PYTHONPATH"] = pythonPath;

                StringBuilder output = new StringBuilder();
                object gate = new object();
                using (Process p = new Process())
                {
                    p.StartInfo = psi;
                    p.OutputDataReceived += delegate(object s, DataReceivedEventArgs e)
                    {
                        if (e.Data == null) return;
                        lock (gate) output.AppendLine(e.Data);
                    };
                    p.ErrorDataReceived += delegate(object s, DataReceivedEventArgs e)
                    {
                        if (e.Data == null) return;
                        lock (gate) output.AppendLine(e.Data);
                    };
                    p.Start();
                    p.BeginOutputReadLine();
                    p.BeginErrorReadLine();
                    if (!p.WaitForExit(timeoutMs))
                    {
                        try { p.Kill(); } catch { }
                        try { p.WaitForExit(2000); } catch { }
                    }
                    else
                    {
                        p.WaitForExit();
                    }
                }
                lock (gate) return output.ToString().Trim();
            }
            catch (Exception ex)
            {
                return "ERR: " + ex.Message;
            }
        }

        /// <summary>Скачивает исходники, портативный Python и ставит зависимости (системный Python не нужен).</summary>
        public static string Prepare(string tag, Action<int, string> progress, out bool ok)
        {
            ok = false;
            TgProxy.InvalidateCache();
            try
            {
                string python = "";
                string refName = string.IsNullOrEmpty(tag) ? "main" : tag;
                string zip = Path.Combine(Path.GetTempPath(), "tgws-" + refName + ".zip");
                string url = string.Format("https://github.com/{0}/archive/refs/{1}.zip", Repo,
                    refName == "main" ? "heads/main" : "tags/" + refName);

                if (progress != null) progress(8, "Скачивание исходников " + refName + "…");
                Web.Download(url, zip, delegate(int percent, long read, long total)
                {
                    if (progress != null) progress(percent < 0 ? 15 : 8 + (int)(percent * 0.22), "Скачивание исходников: " + (percent < 0 ? 0 : percent) + "%");
                }, 180000);

                // Сначала распаковываем в staging: старый рабочий тихий режим не трогаем,
                // пока новый архив не проверен.
                string staging = RootDir + ".new";
                if (Directory.Exists(staging)) Directory.Delete(staging, true);
                Directory.CreateDirectory(staging);
                string tmp = Path.Combine(staging, "unpack");
                Directory.CreateDirectory(tmp);

                if (progress != null) progress(32, "Распаковка…");
                ZipFile.ExtractToDirectory(zip, tmp);
                string found = null;
                foreach (string dir in Directory.GetDirectories(tmp))
                {
                    if (File.Exists(Path.Combine(dir, @"proxy\tg_ws_proxy.py"))) { found = dir; break; }
                }
                if (found == null)
                {
                    try { Directory.Delete(staging, true); } catch { }
                    return "В архиве не найден proxy/tg_ws_proxy.py — похоже, изменилась структура репозитория.";
                }

                if (Directory.Exists(RootDir)) Directory.Delete(RootDir, true);
                Directory.CreateDirectory(RootDir);
                Directory.Move(found, SrcDir);
                try { Directory.Delete(staging, true); } catch { }
                try { File.Delete(zip); } catch { }

                if (progress != null) progress(40, "Готовим портативный Python " + PythonVersion + " (≈10 МБ)…");
                string portableError = EnsurePortable(progress, out python);
                if (portableError.Length > 0) return portableError;

                if (progress != null) progress(70, "Устанавливаем зависимости (httpx, certifi, cryptography)…");
                if (!Directory.Exists(PortableSitePackages) || !HasPackages(python))
                {
                    string pipError = InstallPackages(python, progress);
                    if (pipError.Length > 0) return pipError;
                }

                if (progress != null) progress(92, "Проверяем запуск…");
                string check = Run(PortableConsole, "\"" + ScriptPath + "\" --help", 40000, PortablePythonPath());
                if (check.Trim().Length == 0 || (check.IndexOf("usage", StringComparison.OrdinalIgnoreCase) < 0 && check.IndexOf("error", StringComparison.OrdinalIgnoreCase) >= 0))
                    return "Прокси не запускается из исходников: " + Trim(check);

                File.WriteAllText(MarkerPath,
                    "версия: " + refName + Environment.NewLine +
                    "python: " + python + Environment.NewLine +
                    "портативный: да" + Environment.NewLine +
                    "подготовлено: " + DateTime.Now.ToString("yyyy-MM-dd HH:mm") + Environment.NewLine,
                    new UTF8Encoding(false));

                if (progress != null) progress(100, "Готово");
                ok = true;
                Log.Ok("Тихий режим TG WS Proxy подготовлен (" + refName + ", портативный Python " + PythonVersion + ").");
                return "Готово: исходники " + refName + " и портативный Python установлены в " + RootDir;
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось подготовить тихий режим TG WS Proxy", ex);
                return "Ошибка подготовки: " + ex.Message;
            }
        }

        /// <summary>Путь site-packages портативного Python (для PYTHONPATH).</summary>
        public static string PortablePythonPath()
        {
            return PortableSitePackages;
        }

        /// <summary>Скачивает embeddable-сборку Python с python.org и распаковывает рядом.</summary>
        private static string EnsurePortable(Action<int, string> progress, out string pythonExe)
        {
            pythonExe = PortableConsole;
            if (File.Exists(PortableConsole)) return "";

            string zip = Path.Combine(Path.GetTempPath(), "python-embed-" + PythonVersion + ".zip");
            string url = "https://www.python.org/ftp/python/" + PythonVersion + "/python-" + PythonVersion + "-embed-amd64.zip";
            try
            {
                Web.Download(url, zip, delegate(int percent, long read, long total)
                {
                    if (progress != null) progress(percent < 0 ? 45 : 40 + (int)(percent * 0.18), "Скачивание Python: " + (percent < 0 ? 0 : percent) + "%");
                }, 300000);

                if (Directory.Exists(PortableDir)) Directory.Delete(PortableDir, true);
                Directory.CreateDirectory(PortableDir);
                ZipFile.ExtractToDirectory(zip, PortableDir);
                try { File.Delete(zip); } catch { }

                Directory.CreateDirectory(PortableSitePackages);
                string pth = null;
                foreach (string file in Directory.GetFiles(PortableDir, "python*._pth")) { pth = file; break; }
                if (pth != null)
                {
                    string content = File.ReadAllText(pth);
                    if (content.IndexOf("site-packages", StringComparison.OrdinalIgnoreCase) < 0)
                        content += Environment.NewLine + @"Lib\site-packages" + Environment.NewLine;
                    if (content.IndexOf("import site", StringComparison.OrdinalIgnoreCase) < 0)
                        content += "import site" + Environment.NewLine;
                    File.WriteAllText(pth, content);
                }

                if (!File.Exists(PortableConsole)) return "Не удалось распаковать портативный Python.";
                Log.Ok("Портативный Python " + PythonVersion + " распакован в " + PortableDir);
                return "";
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось скачать портативный Python", ex);
                return "Не удалось скачать Python с python.org: " + ex.Message;
            }
        }

        /// <summary>Ставит pip (get-pip.py) и нужные пакеты в портативный Python.</summary>
        private static string InstallPackages(string python, Action<int, string> progress)
        {
            try
            {
                if (!Directory.Exists(Path.Combine(PortableDir, @"Lib\site-packages\pip")))
                {
                    if (progress != null) progress(72, "Ставим pip в портативный Python…");
                    string getPip = Path.Combine(Path.GetTempPath(), "get-pip.py");
                    Web.Download("https://bootstrap.pypa.io/get-pip.py", getPip, null, 180000);
                    string pipSetup = Run(PortableConsole, "\"" + getPip + "\" --no-warn-script-location --disable-pip-version-check", 300000, PortablePythonPath());
                    try { File.Delete(getPip); } catch { }
                    if (!Directory.Exists(Path.Combine(PortableDir, @"Lib\site-packages\pip")))
                        return "Не удалось установить pip: " + Trim(pipSetup);
                }

                if (progress != null) progress(80, "Устанавливаем пакеты (httpx, certifi, cryptography)…");
                StringBuilder args = new StringBuilder();
                args.Append("-m pip install --no-warn-script-location --disable-pip-version-check --no-input ");
                foreach (string pkg in RequiredPackages) args.Append("\"").Append(pkg).Append("\" ");
                string pipOut = Run(PortableConsole, args.ToString(), 600000, PortablePythonPath());
                if (!HasPackages(python)) return "Не удалось установить зависимости: " + Trim(pipOut);
                return "";
            }
            catch (Exception ex)
            {
                return "Ошибка установки зависимостей: " + ex.Message;
            }
        }

        private static bool HasPackages(string python)
        {
            string output = Run(PortableConsole, "-c \"import httpx, certifi; print(httpx.__version__)\"", 60000, PortablePythonPath());
            return output.Trim().Length > 0 && output.IndexOf("Error", StringComparison.OrdinalIgnoreCase) < 0;
        }

        private static string Run(string exe, string arguments, int timeoutMs, string pythonPath)
        {
            return RunProcess(exe, arguments, timeoutMs, PortableDir, pythonPath);
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
                // Портативный pythonw — основной вариант; системный Python нужен только
                // для совместимости со старыми установками.
                string pythonw;
                if (File.Exists(PortablePython)) pythonw = PortablePython;
                else if (File.Exists(VenvPython)) pythonw = VenvPython;
                else
                {
                    string python = BasePython();
                    if (string.IsNullOrEmpty(python))
                    {
                        error = "Python не найден. Нажмите «Подготовить», чтобы скачать портативный Python.";
                        return false;
                    }
                    pythonw = python;
                    string dir = Path.GetDirectoryName(python);
                    string candidate = string.IsNullOrEmpty(dir) ? "" : Path.Combine(dir, "pythonw.exe");
                    if (!string.IsNullOrEmpty(candidate) && File.Exists(candidate)) pythonw = candidate;
                }

                ProcessStartInfo psi = new ProcessStartInfo(pythonw, args.ToString());
                psi.WorkingDirectory = SrcDir;
                psi.UseShellExecute = false;
                psi.CreateNoWindow = true;
                psi.WindowStyle = ProcessWindowStyle.Hidden;
                string sitePackages = Directory.Exists(PortableSitePackages)
                    ? PortableSitePackages
                    : Path.Combine(VenvDir, @"Lib\site-packages");
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
