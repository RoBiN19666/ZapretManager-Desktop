using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
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
        public string Sha256 = "";
        public DateTime CheckedAt = DateTime.Now;
    }

    /// <summary>
    /// Обновление самого менеджера из GitHub Releases репозитория RoBiN19666/ZapretManager-Desktop.
    /// Версия — тег последнего релиза, файл — ZapretManager.exe из его вложений.
    /// version.txt в ветке main остаётся подсказкой, если редирект релиза недоступен.
    /// </summary>
    public static class SelfUpdate
    {
        public const string Repo = "RoBiN19666/ZapretManager-Desktop";

        /// <summary>Зеркала version.txt на случай, если релизы не отдаются (блокировки, прокси).</summary>
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

        public static string VersionUrl { get { return VersionMirrors[0]; } }

        /// <summary>Тег последнего релиза через редирект /releases/latest (без API-лимитов и кэша CDN).</summary>
        public static string LatestTag()
        {
            try
            {
                string loc = Web.GetRedirectLocation(string.Format("https://github.com/{0}/releases/latest", Repo), 15000);
                if (string.IsNullOrEmpty(loc)) return null;
                int i = loc.LastIndexOf("/tag/", StringComparison.OrdinalIgnoreCase);
                if (i < 0) return null;
                string tag = loc.Substring(i + 5).Trim('/');
                return tag.Length > 0 ? tag : null;
            }
            catch
            {
                return null;
            }
        }

        /// <summary>Адреса файла в релизе: сначала с версией в имени, затем без.</summary>
        public static string[] ExeCandidates(SelfUpdateInfo info)
        {
            List<string> list = new List<string>();
            if (info != null && !string.IsNullOrEmpty(info.Tag))
            {
                if (!string.IsNullOrEmpty(info.Remote))
                {
                    list.Add(string.Format("https://github.com/{0}/releases/download/{1}/ZapretManager-{2}.exe", Repo, info.Tag, info.Remote));
                }
                list.Add(string.Format("https://github.com/{0}/releases/download/{1}/ZapretManager.exe", Repo, info.Tag));
            }
            return list.ToArray();
        }

        /// <summary>Ссылка на checksums.txt релиза (может отсутствовать).</summary>
        private static string ChecksumsUrl(string tag)
        {
            return string.Format("https://github.com/{0}/releases/download/{1}/checksums.txt", Repo, tag);
        }

        public static SelfUpdateInfo Last;

        public static SelfUpdateInfo Check(bool force)
        {
            if (!force && Last != null && (DateTime.Now - Last.CheckedAt).TotalMinutes < 5) return Last;
            SelfUpdateInfo info = new SelfUpdateInfo();
            info.Local = AppInfo.Version;

            string tag = LatestTag();
            string tagVersion = "";
            if (!string.IsNullOrEmpty(tag))
            {
                info.Tag = tag;
                tagVersion = Versions.NormalizeTag(tag);
                if (!Regex.IsMatch(tagVersion, @"^\d+(\.\d+)+(?:[-.][0-9A-Za-z]+)?$")) tagVersion = "";
            }

            // version.txt — резервный источник версии, если релиз недоступен
            string fileVersion = "";
            string lastError = "";
            foreach (string mirror in VersionMirrors)
            {
                try
                {
                    string url = mirror + (mirror.IndexOf('?') >= 0 ? "&" : "?") + "t=" + DateTime.Now.Ticks;
                    string text = Web.GetString(url, 15000).Trim();
                    Match m = Regex.Match(text, @"[0-9]+(?:\.[0-9]+)+(?:[-.][0-9A-Za-z]+)?");
                    if (!m.Success) continue;
                    fileVersion = m.Value;
                    break;
                }
                catch (Exception ex)
                {
                    lastError = ex.Message;
                }
            }

            // главный источник — релиз; version.txt только если релиза нет
            string remote = tagVersion.Length > 0 ? tagVersion : fileVersion;
            if (remote.Length == 0)
            {
                info.Error = lastError.Length > 0 ? lastError : "не удалось определить версию (нет ни релиза, ни version.txt)";
                Last = info;
                return info;
            }

            info.Remote = remote;
            info.Available = Versions.Compare(info.Remote, info.Local) > 0;
            info.ExeUrl = info.Tag != null && info.Tag.Length > 0
                ? string.Format("https://github.com/{0}/releases/download/{1}/ZapretManager.exe", Repo, info.Tag)
                : "";
            info.Ok = true;
            Last = info;
            return info;
        }

        /// <summary>Ожидаемый SHA-256 файла из checksums.txt релиза (пусто, если файла нет).</summary>
        private static string ExpectedSha256(string tag)
        {
            if (string.IsNullOrEmpty(tag)) return "";
            try
            {
                string text = Web.GetString(ChecksumsUrl(tag) + "?t=" + DateTime.Now.Ticks, 15000);
                Match m = Regex.Match(text ?? "", "(?im)^\\s*([0-9a-f]{64})\\s+\\*?(?:ZapretManager[^\\s]*\\.exe)?\\s*$");
                if (m.Success) return m.Groups[1].Value.ToLowerInvariant();
                Match any = Regex.Match(text ?? "", "(?i)\\b([0-9a-f]{64})\\b");
                return any.Success ? any.Groups[1].Value.ToLowerInvariant() : "";
            }
            catch
            {
                return "";
            }
        }

        private static string FileSha256(string path)
        {
            using (SHA256 sha = SHA256.Create())
            using (FileStream fs = File.OpenRead(path))
            {
                byte[] hash = sha.ComputeHash(fs);
                StringBuilder sb = new StringBuilder(hash.Length * 2);
                foreach (byte b in hash) sb.Append(b.ToString("x2"));
                return sb.ToString();
            }
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

                string[] candidates = ExeCandidates(info);
                if (candidates.Length == 0)
                {
                    return "Релиз не найден: обновление качается только из GitHub Releases. Проверьте https://github.com/" + Repo + "/releases";
                }

                string exePath = Application.ExecutablePath;
                string dir = Path.Combine(Path.GetTempPath(), "ZapretManagerUpdate");
                if (!Directory.Exists(dir)) Directory.CreateDirectory(dir);
                string newExe = Path.Combine(dir, "ZapretManager-" + info.Remote + ".exe");

                if (progress != null) progress(3, "Скачивание обновления " + info.Remote + "…");
                string expected = ExpectedSha256(info.Tag);
                if (expected.Length > 0) info.Sha256 = expected;
                string downloadError = "";
                bool downloaded = false;
                foreach (string url in candidates)
                {
                    try
                    {
                        if (File.Exists(newExe)) File.Delete(newExe);
                        Web.Download(url, newExe, delegate(int percent, long read, long total)
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
                        if (expected.Length > 0)
                        {
                            string got = FileSha256(newExe);
                            if (!string.Equals(got, expected, StringComparison.OrdinalIgnoreCase))
                            {
                                downloadError = "не совпала контрольная сумма (ждём " + expected.Substring(0, 12) + "…, получили " + got.Substring(0, 12) + "…)";
                                Log.Warn("Самообновление: " + downloadError);
                                continue;
                            }
                            Log.Info("Самообновление: SHA-256 совпал (" + expected.Substring(0, 16) + "…)");
                        }
                        else
                        {
                            Log.Warn("Самообновление: в релизе нет checksums.txt, проверяю только размер и версию файла.");
                        }
                        string gotVersion = FileVersionInfo.GetVersionInfo(newExe).FileVersion;
                        if (!string.IsNullOrEmpty(gotVersion) && Versions.Compare(gotVersion, info.Remote) < 0)
                        {
                            downloadError = "файл в релизе старее заявленной версии (" + gotVersion + ")";
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
                try
                {
                    // Новый exe сам дождётся выхода старого процесса, скопирует себя
                    // на место старого и запустит его. Это надёжнее cmd.exe: пути
                    // передаются через Unicode-переменные окружения, без кодировок и кавычек.
                    ProcessStartInfo psi = new ProcessStartInfo(newExe);
                    psi.UseShellExecute = false;
                    psi.CreateNoWindow = true;
                    psi.WindowStyle = ProcessWindowStyle.Hidden;
                    psi.WorkingDirectory = dir;
                    psi.Arguments = "--apply-update";
                    psi.EnvironmentVariables["ZAPRET_UPDATE_TARGET"] = exePath;
                    psi.EnvironmentVariables["ZAPRET_UPDATE_PID"] = Process.GetCurrentProcess().Id.ToString();
                    Process.Start(psi);
                }
                catch (Exception ex)
                {
                    return "Не удалось запустить замену файла: " + ex.Message;
                }

                ok = true;
                pendingPath = newExe;
                Log.Ok("Самообновление: скачано " + info.Remote + " из релиза " + info.Tag + ", замена выполнится после выхода.");
                return "Обновление " + info.Remote + " скачано из релиза. Приложение закроется и запустится заново уже новой версией.";
            }
            catch (Exception ex)
            {
                Log.Error("Ошибка самообновления", ex);
                return "Ошибка самообновления: " + ex.Message;
            }
        }

        /// <summary>
        /// Выполняется новым exe (флаг --apply-update): ждёт выхода старого процесса,
        /// копирует себя на место старого exe и запускает его.
        /// </summary>
        public static void RunApplyUpdate()
        {
            string dir = Path.Combine(Path.GetTempPath(), "ZapretManagerUpdate");
            try { if (!Directory.Exists(dir)) Directory.CreateDirectory(dir); } catch { }
            string resultFile = Path.Combine(dir, "apply-result.txt");
            try
            {
                string target = Environment.GetEnvironmentVariable("ZAPRET_UPDATE_TARGET");
                string pidText = Environment.GetEnvironmentVariable("ZAPRET_UPDATE_PID");
                if (string.IsNullOrEmpty(target))
                {
                    File.WriteAllText(resultFile, "no target", Encoding.UTF8);
                    return;
                }

                int pid;
                if (int.TryParse(pidText, out pid) && pid > 0)
                {
                    try
                    {
                        using (Process old = Process.GetProcessById(pid))
                        {
                            if (!old.WaitForExit(120000))
                            {
                                File.WriteAllText(resultFile, "old process still running", Encoding.UTF8);
                                return;
                            }
                        }
                    }
                    catch { }
                }

                // Даём Windows отпустить exe и antivirus'у закончить проверку.
                string source = Application.ExecutablePath;
                Exception last = null;
                for (int i = 0; i < 30; i++)
                {
                    try
                    {
                        File.Copy(source, target, true);
                        last = null;
                        break;
                    }
                    catch (Exception ex)
                    {
                        last = ex;
                        Thread.Sleep(500);
                    }
                }

                if (last != null)
                {
                    File.WriteAllText(resultFile, "copy failed: " + last.Message, Encoding.UTF8);
                    // не оставляем пользователя без приложения: запускаем старый exe
                    try { Process.Start(new ProcessStartInfo(target) { UseShellExecute = true, WorkingDirectory = Path.GetDirectoryName(target) }); } catch { }
                    return;
                }

                File.WriteAllText(resultFile, "ok " + DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss"), Encoding.UTF8);
                try { Process.Start(new ProcessStartInfo(target) { UseShellExecute = true, WorkingDirectory = Path.GetDirectoryName(target) }); } catch { }
            }
            catch (Exception ex)
            {
                try { File.WriteAllText(resultFile, "error: " + ex.Message, Encoding.UTF8); } catch { }
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
