using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows.Forms;

namespace ZapretManager
{
    public sealed class SelfUpdateInfo
    {
        public bool Ok;
        public string Local = "";
        public string Remote = "";
        public bool Available;
        public string Error = "";
        public string ExeUrl = "";
        public string Tag = "";
        public DateTime CheckedAt = DateTime.Now;
    }

    /// <summary>
    /// Обновление самого менеджера из репозитория RoBiN19666/ZapretManager-Windows.
    /// Версия берётся из version.txt в ветке main, файл — ZapretManager.exe оттуда же.
    /// </summary>
    public static class SelfUpdate
    {
        public const string Repo = "RoBiN19666/ZapretManager-Windows";

        /// <summary>Зеркала version.txt: raw отдаёт свежий коммит не сразу, jsDelivr — почти сразу.</summary>
        public static string[] VersionMirrors
        {
            get
            {
                return new string[]
                {
                    string.Format("https://raw.githubusercontent.com/{0}/main/version.txt", Repo),
                    string.Format("https://cdn.jsdelivr.net/gh/{0}@main/version.txt", Repo),
                    string.Format("https://github.com/{0}/raw/main/version.txt", Repo)
                };
            }
        }

        public static string[] ExeMirrors
        {
            get
            {
                return new string[]
                {
                    string.Format("https://raw.githubusercontent.com/{0}/main/ZapretManager.exe", Repo),
                    string.Format("https://cdn.jsdelivr.net/gh/{0}@main/ZapretManager.exe", Repo),
                    string.Format("https://github.com/{0}/raw/main/ZapretManager.exe", Repo)
                };
            }
        }

        public static string VersionUrl { get { return VersionMirrors[0]; } }
        public static string ExeUrl { get { return ExeMirrors[0]; } }

        /// <summary>Тег последнего релиза через редирект /releases/latest (без API-лимитов и кэша CDN).</summary>
        public static string LatestTag()
        {
            try
            {
                string loc = Web.GetRedirectLocation(string.Format("https://github.com/{0}/releases/latest", Repo), 15000);
                if (string.IsNullOrEmpty(loc)) return null;
                int i = loc.LastIndexOf("/tag/", StringComparison.OrdinalIgnoreCase);
                if (i < 0) return null;
                return loc.Substring(i + 5).Trim('/');
            }
            catch
            {
                return null;
            }
        }

        /// <summary>Кандидаты на скачивание: сначала файл из релиза, потом ветка main.</summary>
        public static string[] ExeCandidates(SelfUpdateInfo info)
        {
            System.Collections.Generic.List<string> list = new System.Collections.Generic.List<string>();
            if (info != null && !string.IsNullOrEmpty(info.Tag))
            {
                list.Add(string.Format("https://github.com/{0}/releases/download/{1}/ZapretManager.exe", Repo, info.Tag));
                list.Add(string.Format("https://github.com/{0}/releases/download/{1}/ZapretManager-{2}.exe", Repo, info.Tag, info.Remote));
            }
            list.AddRange(ExeMirrors);
            return list.ToArray();
        }

        public static SelfUpdateInfo Last;

        public static SelfUpdateInfo Check(bool force)
        {
            if (!force && Last != null && (DateTime.Now - Last.CheckedAt).TotalMinutes < 5) return Last;
            SelfUpdateInfo info = new SelfUpdateInfo();
            info.Local = AppInfo.Version;
            // тег релиза + version.txt: часть CDN отдаёт кэш, поэтому берём максимальную версию
            string lastError = "";
            string best = "";
            string tag = LatestTag();
            if (!string.IsNullOrEmpty(tag))
            {
                info.Tag = tag;
                Match tm = Regex.Match(tag, "[0-9]+(\\.[0-9]+)*");
                if (tm.Success) best = tm.Value;
            }
            foreach (string mirror in VersionMirrors)
            {
                try
                {
                    string url = mirror + (mirror.IndexOf('?') >= 0 ? "&" : "?") + "t=" + DateTime.Now.Ticks;
                    string text = Web.GetString(url, 15000).Trim();
                    Match m = Regex.Match(text, "[0-9]+(\\.[0-9]+)*");
                    if (!m.Success) continue;
                    if (best.Length == 0 || Versions.Compare(m.Value, best) > 0) best = m.Value;
                }
                catch (Exception ex)
                {
                    lastError = ex.Message;
                }
            }
            if (best.Length > 0)
            {
                info.Remote = best;
                info.Available = Versions.Compare(info.Remote, info.Local) > 0;
                info.ExeUrl = !string.IsNullOrEmpty(info.Tag)
                    ? string.Format("https://github.com/{0}/releases/download/{1}/ZapretManager.exe", Repo, info.Tag)
                    : ExeUrl;
                info.Ok = true;
            }
            else
            {
                info.Error = lastError.Length > 0 ? lastError : "не удалось получить файл версии";
            }
            Last = info;
            return info;
        }

        /// <summary>
        /// Скачивает новую версию и подготавливает замену файла после выхода приложения.
        /// Возвращает текст результата; ok=false при ошибке.
        /// </summary>
        public static string Apply(SelfUpdateInfo info, Action<int, string> progress, out bool ok, out string pendingPath)
        {
            ok = false;
            pendingPath = "";
            try
            {
                if (info == null || !info.Ok) return "Сначала проверьте обновления.";
                if (!info.Available) return "Установлена последняя версия " + info.Local + ".";

                string exePath = Application.ExecutablePath;
                string dir = Path.Combine(Path.GetTempPath(), "ZapretManagerUpdate");
                if (!Directory.Exists(dir)) Directory.CreateDirectory(dir);
                string newExe = Path.Combine(dir, "ZapretManager-" + info.Remote + ".exe");

                if (progress != null) progress(3, "Скачивание обновления " + info.Remote + "…");
                string downloadError = "";
                bool downloaded = false;
                foreach (string mirror in ExeCandidates(info))
                {
                    try
                    {
                        if (File.Exists(newExe)) File.Delete(newExe);
                        Web.Download(mirror, newExe, delegate(int percent, long read, long total)
                        {
                            if (progress != null) progress(percent < 0 ? 20 : 3 + (int)(percent * 0.8), "Скачивание: " + (percent < 0 ? 0 : percent) + "%");
                        }, 240000);

                        FileInfo fi = new FileInfo(newExe);
                        if (!fi.Exists || fi.Length < 200 * 1024)
                        {
                            downloadError = "файл слишком мал (" + (fi.Exists ? fi.Length : 0) + " байт)";
                            continue;
                        }
                        using (FileStream fs = File.OpenRead(newExe))
                        {
                            if (fs.ReadByte() != 'M' || fs.ReadByte() != 'Z') { downloadError = "файл не является программой"; continue; }
                        }
                        string got = FileVersionInfo.GetVersionInfo(newExe).FileVersion;
                        if (!string.IsNullOrEmpty(got) && Versions.Compare(got, info.Remote) < 0)
                        {
                            downloadError = "зеркало отдало старую версию " + got;
                            continue;
                        }
                        downloaded = true;
                        break;
                    }
                    catch (Exception ex)
                    {
                        downloadError = ex.Message;
                    }
                }
                if (!downloaded) return "Не удалось скачать обновление: " + downloadError;

                if (progress != null) progress(90, "Подготовка замены файла…");
                string script = Path.Combine(dir, "apply-update.cmd");
                StringBuilder sb = new StringBuilder();
                sb.AppendLine("@echo off");
                sb.AppendLine(":wait");
                sb.AppendLine("tasklist /FI \"IMAGENAME eq ZapretManager.exe\" 2>nul | find /I \"ZapretManager.exe\" >nul");
                sb.AppendLine("if not errorlevel 1 (");
                sb.AppendLine("  timeout /t 1 /nobreak >nul");
                sb.AppendLine("  goto wait");
                sb.AppendLine(")");
                sb.AppendLine("copy /y \"" + newExe + "\" \"" + exePath + "\" >nul");
                sb.AppendLine("if errorlevel 1 (");
                sb.AppendLine("  del \"%~f0\"");
                sb.AppendLine("  exit /b 1");
                sb.AppendLine(")");
                sb.AppendLine("start \"\" \"" + exePath + "\"");
                sb.AppendLine("del \"%~f0\"");
                File.WriteAllText(script, sb.ToString(), Encoding.Default);

                if (progress != null) progress(96, "Запуск обновления…");
                ProcessStartInfo psi = new ProcessStartInfo("cmd.exe", "/c \"" + script + "\"");
                psi.UseShellExecute = false;
                psi.CreateNoWindow = true;
                psi.WindowStyle = ProcessWindowStyle.Hidden;
                psi.WorkingDirectory = dir;
                Process.Start(psi);

                ok = true;
                pendingPath = newExe;
                Log.Ok("Самообновление: скачано " + info.Remote + ", замена выполнится после выхода.");
                return "Обновление " + info.Remote + " скачано. Приложение закроется и запустится заново уже новой версией.";
            }
            catch (Exception ex)
            {
                Log.Error("Ошибка самообновления", ex);
                return "Ошибка самообновления: " + ex.Message;
            }
        }

        /// <summary>Текст версии для интерфейса: "1.1.0 → 1.2.0".</summary>
        public static string Describe(SelfUpdateInfo info)
        {
            if (info == null) return "";
            if (!info.Ok) return "не удалось проверить (" + info.Error + ")";
            if (!info.Available) return "актуальная версия " + info.Local;
            return "доступна версия " + info.Remote;
        }
    }
}
