using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Net.NetworkInformation;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Web.Script.Serialization;

namespace ZapretManager
{
    /// <summary>Состояние TG WS Proxy (локальный MTProto-прокси для Telegram).</summary>
    public sealed class TgProxyState
    {
        public bool Installed;
        public string Dir = "";
        public string ExePath = "";
        public string Version = "";
        public bool Running;
        public int InstanceCount;
        public int Pid;
        public DateTime? Started;
        public bool PortListening;
        public int Port = 1443;
        public string Host = "127.0.0.1";
        public string Secret = "";
        public string ConfigPath = "";
        public string ConfigError = "";
        public bool QuietMode;
        public bool QuietPrepared;
        public string AppDataDir
        {
            get { return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "TgWsProxy"); }
        }

        public string Link
        {
            get
            {
                if (string.IsNullOrEmpty(Secret)) return "";
                return string.Format("tg://proxy?server={0}&port={1}&secret={2}", Host, Port, Secret);
            }
        }

        public string StatusText
        {
            get
            {
                if (!Installed) return "не установлен";
                if (!Running) return "остановлен";
                return PortListening ? "работает" : "запускается";
            }
        }
    }

    /// <summary>Управление TG WS Proxy: установка, обновление, запуск и остановка.</summary>
    public static class TgProxy
    {
        public const string Repo = "Flowseal/tg-ws-proxy";

        public static string DefaultDir
        {
            get
            {
                string baseDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "ZapretManager");
                return Path.Combine(Path.Combine(baseDir, "components"), "tgwsproxy");
            }
        }

        public static string AssetName()
        {
            try
            {
                string arch = (Environment.GetEnvironmentVariable("PROCESSOR_ARCHITECTURE") ?? "").ToUpperInvariant();
                if (arch.IndexOf("ARM64") >= 0) return "TgWsProxy_windows_arm64.exe";
            }
            catch { }
            return "TgWsProxy_windows.exe";
        }

        public static string DownloadUrl(string tag)
        {
            return string.Format("https://github.com/{0}/releases/download/{1}/{2}", Repo, tag, AssetName());
        }

        // ---------- состояние ----------

        /// <summary>Версия из ресурсов exe (PyInstaller заполняет FileVersion).</summary>
        public static string ReadExeVersion(string path)
        {
            try
            {
                if (!File.Exists(path)) return "";
                FileVersionInfo vi = FileVersionInfo.GetVersionInfo(path);
                string v = vi.FileVersion;
                if (string.IsNullOrEmpty(v)) v = vi.ProductVersion;
                if (string.IsNullOrEmpty(v)) return "";
                v = v.Trim();
                while (v.EndsWith(".0") && v.Split('.').Length > 3) v = v.Substring(0, v.Length - 2);
                return v;
            }
            catch { return ""; }
        }

        /// <summary>Путь к exe запущенного прокси (по командной строке процесса).</summary>
        private static string RunningExePath()
        {
            foreach (Proc.ProcInfo pi in FindProcesses())
            {
                string exe = ExtractExePath(pi.CommandLine);
                if (!string.IsNullOrEmpty(exe)) return exe;
            }
            return null;
        }

        /// <summary>Путь к exe из командной строки процесса.</summary>
        private static string ExtractExePath(string cmd)
        {
            if (string.IsNullOrEmpty(cmd)) return "";
            cmd = cmd.Trim();
            if (cmd.StartsWith("\""))
            {
                int end = cmd.IndexOf('"', 1);
                if (end > 1) return cmd.Substring(1, end - 1);
            }
            int space = cmd.IndexOf(' ');
            return space > 0 ? cmd.Substring(0, space) : cmd;
        }

        /// <summary>Останавливает только те экземпляры, чей exe лежит в указанной папке.</summary>
        private static void StopProcessesInside(string dir)
        {
            if (string.IsNullOrEmpty(dir)) return;
            string full = "";
            try { full = Path.GetFullPath(dir).TrimEnd('\\'); } catch { return; }
            foreach (Proc.ProcInfo pi in FindProcesses())
            {
                string exe = ExtractExePath(pi.CommandLine);
                if (string.IsNullOrEmpty(exe)) continue;
                string exeDir = "";
                try { exeDir = Path.GetFullPath(Path.GetDirectoryName(exe)).TrimEnd('\\'); } catch { }
                if (!string.Equals(exeDir, full, StringComparison.OrdinalIgnoreCase)) continue;
                try
                {
                    using (Process p = Process.GetProcessById(pi.Pid))
                    {
                        if (!p.CloseMainWindow() || !p.WaitForExit(2500)) p.Kill();
                    }
                    Log.Info("Остановлен TG WS Proxy из " + dir + " перед обновлением.");
                }
                catch (Exception ex) { Log.Warn("Не удалось остановить TG WS Proxy: " + ex.Message); }
            }
        }

        public static string StartupFolderExe
        {
            get
            {
                try
                {
                    string dir = Environment.GetFolderPath(Environment.SpecialFolder.Startup);
                    if (string.IsNullOrEmpty(dir) || !Directory.Exists(dir)) return null;
                    string[] files = Directory.GetFiles(dir, "TgWsProxy*.exe");
                    return files.Length > 0 ? files[0] : null;
                }
                catch { return null; }
            }
        }

        public static TgProxyState GetState(AppConfig config)
        {
            TgProxyState st = new TgProxyState();
            string dir = config.Get("TgDir", "");
            string exe = config.Get("TgExe", "");
            if (exe.Length > 0 && File.Exists(exe))
            {
                st.ExePath = exe;
                st.Dir = Path.GetDirectoryName(exe);
            }
            else if (dir.Length > 0 && Directory.Exists(dir))
            {
                string found = FindExe(dir);
                if (found != null) { st.ExePath = found; st.Dir = dir; }
            }
            if (st.ExePath.Length == 0 && Directory.Exists(DefaultDir))
            {
                string found = FindExe(DefaultDir);
                if (found != null) { st.ExePath = found; st.Dir = DefaultDir; }
            }
            // Подхватываем уже установленную копию: сначала запущенный процесс, потом папка «Автозагрузка»
            if (st.ExePath.Length == 0)
            {
                string running = RunningExePath();
                if (!string.IsNullOrEmpty(running) && File.Exists(running))
                {
                    st.ExePath = running;
                    st.Dir = Path.GetDirectoryName(running);
                }
            }
            if (st.ExePath.Length == 0)
            {
                string startup = StartupFolderExe;
                if (!string.IsNullOrEmpty(startup) && File.Exists(startup))
                {
                    st.ExePath = startup;
                    st.Dir = Path.GetDirectoryName(startup);
                }
            }
            st.Installed = st.ExePath.Length > 0 && File.Exists(st.ExePath);
            if (st.Installed && config != null && !string.Equals(config.Get("TgExe", ""), st.ExePath, StringComparison.OrdinalIgnoreCase))
            {
                config.Set("TgExe", st.ExePath);
                config.Set("TgDir", st.Dir);
                config.Set("TgVersion", ReadExeVersion(st.ExePath));
                config.Save();
            }
            st.Version = st.Installed ? ReadExeVersion(st.ExePath) : "";
            if (st.Version.Length == 0) st.Version = config.Get("TgVersion", "");
            if (st.Installed && st.Version.Length == 0) st.Version = ReadVersionFile(st.Dir);

            ReadConfig(st);
            List<Proc.ProcInfo> procs = FindProcesses();
            st.InstanceCount = procs.Count;
            if (procs.Count > 0)
            {
                st.Running = true;
                // основной — тот, что занимает больше памяти (реально обслуживает соединения)
                Proc.ProcInfo primary = procs[0];
                foreach (Proc.ProcInfo pi in procs)
                {
                    if (pi.WorkingSet > primary.WorkingSet) primary = pi;
                }
                st.Pid = primary.Pid;
                st.Started = primary.Started;
            }
st.PortListening = IsPortListening(st.Port);

            // Тихий режим: работаем из исходников, без иконки в трее
            if (RunMode(config) == "quiet")
            {
                st.QuietMode = true;
                st.QuietPrepared = TgSource.IsPrepared;
                List<Proc.ProcInfo> quietProcs = TgSource.Find();
                if (quietProcs.Count > 0)
                {
                    st.Running = true;
                    st.InstanceCount = quietProcs.Count;
                    st.Pid = quietProcs[0].Pid;
                    st.Started = quietProcs[0].Started;
                }
                if (st.QuietPrepared)
                {
                    st.Installed = true;
                    st.Dir = TgSource.RootDir;
                    st.ExePath = TgSource.ScriptPath;
                    st.Version = TgSource.PreparedVersion();
                }
            }
            return st;
        }

        private static string FindExe(string dir)
        {
            try
            {
                string[] candidates = Directory.GetFiles(dir, "TgWsProxy*.exe");
                if (candidates.Length > 0) return candidates[0];
                string direct = Path.Combine(dir, "TgWsProxy_windows.exe");
                if (File.Exists(direct)) return direct;
            }
            catch { }
            return null;
        }

        private static string ReadVersionFile(string dir)
        {
            try
            {
                string file = Path.Combine(dir, "version.txt");
                if (File.Exists(file)) return File.ReadAllText(file).Trim();
            }
            catch { }
            return "";
        }

        private static void ReadConfig(TgProxyState st)
        {
            st.ConfigPath = Path.Combine(st.AppDataDir, "config.json");
            try
            {
                if (!File.Exists(st.ConfigPath))
                {
                    st.ConfigError = "файл конфигурации ещё не создан (появится после первого запуска)";
                    return;
                }
                JavaScriptSerializer js = new JavaScriptSerializer();
                Dictionary<string, object> json = js.DeserializeObject(File.ReadAllText(st.ConfigPath, Encoding.UTF8)) as Dictionary<string, object>;
                if (json == null) { st.ConfigError = "не удалось разобрать config.json"; return; }
                object v;
                if (json.TryGetValue("port", out v) && v != null) st.Port = Convert.ToInt32(v);
                if (json.TryGetValue("host", out v) && v != null) st.Host = Convert.ToString(v);
                if (json.TryGetValue("secret", out v) && v != null) st.Secret = Convert.ToString(v);
            }
            catch (Exception ex)
            {
                st.ConfigError = ex.Message;
            }
        }

        public static bool IsPortListening(int port)
        {
            try
            {
                IPGlobalProperties props = IPGlobalProperties.GetIPGlobalProperties();
                foreach (IPEndPoint ep in props.GetActiveTcpListeners())
                {
                    if (ep.Port == port) return true;
                }
            }
            catch { }
            return false;
        }

        public static List<Proc.ProcInfo> FindProcesses()
        {
            List<Proc.ProcInfo> list = new List<Proc.ProcInfo>();
            try
            {
                foreach (Proc.ProcInfo p in Proc.ListLike("TgWsProxy%"))
                {
                    list.Add(p);
                }
            }
            catch { }
            return list;
        }

        // ---------- запуск / остановка ----------

        /// <summary>Режим запуска: exe (официальный файл) или quiet (исходники без трея).</summary>
        public static string RunMode(AppConfig config)
        {
            return string.Equals(config.Get("TgRunMode", "exe"), "quiet", StringComparison.OrdinalIgnoreCase) ? "quiet" : "exe";
        }

        public static void SetRunMode(AppConfig config, string mode)
        {
            config.Set("TgRunMode", string.Equals(mode, "quiet", StringComparison.OrdinalIgnoreCase) ? "quiet" : "exe");
            config.Save();
        }

        public static string Start(AppConfig config, out bool ok)
        {
            ok = false;
            bool quiet = RunMode(config) == "quiet";
            if (quiet)
            {
                TgProxyState quietState = GetState(config);
                string quietError;
                int quietPid;
                if (!TgSource.Start(quietState, out quietPid, out quietError))
                {
                    Log.Warn("Тихий режим TG WS Proxy: " + quietError);
                    return quietError;
                }
                for (int i = 0; i < 20; i++)
                {
                    Thread.Sleep(300);
                    TgProxyState check = GetState(config);
                    if (check.PortListening) { ok = true; break; }
                }
                TgProxyState after = GetState(config);
                ok = after.Running;
                if (ok)
                    return string.Format("TG WS Proxy запущен без трея: {0}:{1} (PID {2}). Подключение — кнопкой «Открыть в Telegram».",
                        after.Host, after.Port, after.Pid);
                return "Процесс запущен, но порт " + after.Port + " ещё не слушается. Проверьте журнал " + TgSource.LogPath;
            }

            TgProxyState st = GetState(config);
            if (!st.Installed) return "TG WS Proxy не установлен. Установите его на странице «TG-прокси».";
            if (st.Running)
            {
                ok = true;
                return "TG WS Proxy уже запущен (PID " + st.Pid + ").";
            }
            try
            {
                ProcessStartInfo psi = new ProcessStartInfo(st.ExePath);
                psi.WorkingDirectory = st.Dir;
                psi.UseShellExecute = true;
                Process.Start(psi);
                for (int i = 0; i < 20; i++)
                {
                    Thread.Sleep(300);
                    if (FindProcesses().Count > 0) break;
                }
                Thread.Sleep(700);
                TgProxyState after = GetState(config);
                if (after.Running)
                {
                    ok = true;
                    Log.Ok("TG WS Proxy запущен (PID " + after.Pid + ").");
                    return string.Format("TG WS Proxy запущен: {0}:{1}. Подключение в Telegram — кнопкой «Открыть в Telegram».", after.Host, after.Port);
                }
                Log.Warn("TG WS Proxy: процесс не обнаружен после запуска.");
                return "Файл запущен, но процесс TgWsProxy не найден. Проверьте журнал приложения и антивирус.";
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось запустить TG WS Proxy", ex);
                return "Не удалось запустить TG WS Proxy: " + ex.Message;
            }
        }

        public static string Stop()
        {
            int quietStopped = TgSource.Stop();
            if (quietStopped > 0) return "TG WS Proxy (тихий режим) остановлен.";
            List<Proc.ProcInfo> procs = FindProcesses();
            if (procs.Count == 0) return "TG WS Proxy уже остановлен.";
            int closed = 0;
            foreach (Proc.ProcInfo pi in procs)
            {
                try
                {
                    using (Process p = Process.GetProcessById(pi.Pid))
                    {
                        if (p.CloseMainWindow() && p.WaitForExit(2500)) { closed++; continue; }
                        p.Kill();
                        p.WaitForExit(2500);
                        closed++;
                    }
                }
                catch (Exception ex)
                {
                    Log.Warn("Не удалось остановить TG WS Proxy (PID " + pi.Pid + "): " + ex.Message);
                }
            }
            Log.Ok("TG WS Proxy остановлен (процессов закрыто: " + closed + ").");
            return closed > 0 ? "TG WS Proxy остановлен." : "Не удалось остановить TG WS Proxy.";
        }

        public static string OpenInTelegram(AppConfig config)
        {
            TgProxyState st = GetState(config);
            if (string.IsNullOrEmpty(st.Link))
                return "Ссылка ещё не сформирована: запустите TG WS Proxy, чтобы он создал секрет в config.json.";
            try
            {
                Process.Start(new ProcessStartInfo(st.Link) { UseShellExecute = true });
                return "Ссылка открыта в Telegram.";
            }
            catch (Exception ex)
            {
                return "Не удалось открыть ссылку: " + ex.Message;
            }
        }

        // ---------- обновление ----------

        /// <summary>Последний известный релиз (заполняется любой проверкой обновлений).</summary>
        public static ReleaseInfo LastKnownLatest;

        public static ReleaseInfo CheckLatest()
        {
            string tag = GitHub.LatestTagByRedirect(Repo);
            if (string.IsNullOrEmpty(tag)) throw new ApplicationException("Не удалось определить последнюю версию TG WS Proxy.");
            ReleaseInfo rel = new ReleaseInfo();
            rel.Tag = tag;
            rel.Name = "TG WS Proxy " + tag;
            rel.HtmlUrl = string.Format("https://github.com/{0}/releases/tag/{1}", Repo, tag);
            AssetInfo asset = new AssetInfo();
            asset.Name = AssetName();
            asset.Url = DownloadUrl(tag);
            rel.Assets.Add(asset);
            rel.Body = "Описание изменений: " + rel.HtmlUrl;
            LastKnownLatest = rel;
            return rel;
        }

        /// <summary>Скачивает и устанавливает указанную версию. Возвращает текст результата.</summary>
        public static string Install(AppConfig config, string tag, string targetDir, Action<int, string> progress, out bool ok)
        {
            ok = false;
            try
            {
                if (string.IsNullOrEmpty(targetDir)) targetDir = DefaultDir;
                config.Set("TgDir", targetDir);
                // останавливаем только экземпляры из обновляемой папки, чужие копии не трогаем
                StopProcessesInside(targetDir);

                string tmp = Path.Combine(AppPaths.TempDir, "tgwsproxy-" + tag + ".exe");
                if (progress != null) progress(2, "Скачивание TG WS Proxy " + tag + " (~21 МБ)…");
                Web.Download(DownloadUrl(tag), tmp, delegate(int percent, long read, long total)
                {
                    if (progress != null) progress(percent < 0 ? 20 : 2 + (int)(percent * 0.8), "Скачивание TG WS Proxy: " + (percent < 0 ? 0 : percent) + "%");
                }, 300000);

                FileInfo fi = new FileInfo(tmp);
                if (!fi.Exists || fi.Length < 5 * 1024 * 1024)
                    return "Скачанный файл слишком мал (" + (fi.Exists ? fi.Length : 0) + " байт) — вероятно, ошибка загрузки или блокировка антивирусом.";
                if (!LooksLikeExe(tmp)) return "Скачанный файл не является исполняемым (возможно, страница ошибки вместо файла).";

                if (progress != null) progress(85, "Установка…");
                Directory.CreateDirectory(targetDir);
                string target = Path.Combine(targetDir, AssetName());
                if (File.Exists(target))
                {
                    try { File.Delete(target); }
                    catch
                    {
                        string moved = target + ".old-" + DateTime.Now.ToString("HHmmss");
                        File.Move(target, moved);
                    }
                }
                File.Copy(tmp, target, true);
                File.WriteAllText(Path.Combine(targetDir, "version.txt"), tag, new UTF8Encoding(false));
                try { File.Delete(tmp); } catch { }

                config.Set("TgExe", target);
                config.Set("TgVersion", tag);
                config.Save();
                if (progress != null) progress(100, "Готово");
                Log.Ok("TG WS Proxy " + tag + " установлен в " + targetDir);
                ok = true;
                return "TG WS Proxy " + tag + " установлен: " + target;
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось установить TG WS Proxy", ex);
                return "Ошибка установки TG WS Proxy: " + ex.Message;
            }
        }

        private static bool LooksLikeExe(string path)
        {
            try
            {
                using (FileStream fs = File.OpenRead(path))
                {
                    int b1 = fs.ReadByte();
                    int b2 = fs.ReadByte();
                    return b1 == 'M' && b2 == 'Z';
                }
            }
            catch { return false; }
        }

        public static bool UpdateAvailable(TgProxyState st, ReleaseInfo latest)
        {
            if (latest == null || string.IsNullOrEmpty(st.Version)) return false;
            return Versions.Compare(latest.Tag, st.Version) > 0;
        }

        public static string ConfigPathForApp
        {
            get { return Path.Combine(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "TgWsProxy"), "config.json"); }
        }

        public static string LogPath
        {
            get { return Path.Combine(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "TgWsProxy"), "proxy.log"); }
        }
    }
}
