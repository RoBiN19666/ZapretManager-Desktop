using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.IO.Compression;
using System.Net;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Web.Script.Serialization;

namespace ZapretManager
{
    public sealed class AssetInfo
    {
        public string Name = "";
        public string Url = "";
        public long Size;
    }

    public sealed class ReleaseInfo
    {
        public string Tag = "";
        public string Name = "";
        public string Body = "";
        public DateTime Published = DateTime.MinValue;
        public string HtmlUrl = "";
        public List<AssetInfo> Assets = new List<AssetInfo>();

        public AssetInfo FindZip()
        {
            foreach (AssetInfo a in Assets)
            {
                if (a.Name.EndsWith(".zip", StringComparison.OrdinalIgnoreCase)) return a;
            }
            return null;
        }

        public string CleanBody()
        {
            string b = Body == null ? "" : Body;
            b = b.Replace("\r\n", "\n").Replace("\r", "\n");
            b = Regex.Replace(b, "<!--.*?-->", "", RegexOptions.Singleline);
            b = Regex.Replace(b, "<[^>]+>", "");
            return b.Trim();
        }
    }

    public static class GitHub
    {
        public const string OfficialRepo = "Flowseal/zapret-discord-youtube";
        public const string ForkRepo = "RoBiN19666/zapret-discord-youtube";
        public const string VersionUrl = "https://raw.githubusercontent.com/Flowseal/zapret-discord-youtube/main/.service/version.txt";
        public const string ReleasesPage = "https://github.com/Flowseal/zapret-discord-youtube/releases";

        public static ReleaseInfo GetLatestRelease(string repo)
        {
            string url = string.Format("https://api.github.com/repos/{0}/releases?per_page=1", repo);
            string json = Web.GetString(url, 15000);
            JavaScriptSerializer js = new JavaScriptSerializer();
            js.MaxJsonLength = 16 * 1024 * 1024;
            object parsed = js.DeserializeObject(json);
            object[] arr = parsed as object[];
            if (arr == null || arr.Length == 0) throw new ApplicationException("GitHub не вернул ни одного релиза.");
            Dictionary<string, object> rel = arr[0] as Dictionary<string, object>;
            if (rel == null) throw new ApplicationException("Не удалось разобрать ответ GitHub.");
            return ToRelease(rel);
        }

        /// <summary>
        /// Тег последнего релиза без GitHub API (по редиректу /releases/latest) —
        /// не тратит лимит запросов и работает даже при 403.
        /// </summary>
        public static string LatestTagByRedirect(string repo)
        {
            try
            {
                string url = string.Format("https://github.com/{0}/releases/latest", repo);
                string location = Web.GetRedirectLocation(url, 15000);
                if (string.IsNullOrEmpty(location)) return null;
                Match m = Regex.Match(location, "/tag/([^/?#]+)");
                if (!m.Success) return null;
                return Uri.UnescapeDataString(m.Groups[1].Value);
            }
            catch (Exception ex)
            {
                Log.Warn("Не удалось получить последний тег " + repo + ": " + ex.Message);
                return null;
            }
        }

        /// <summary>
        /// Резервный источник: version.txt в репозитории (как в service.bat).
        /// Нужен, когда GitHub API отвечает 403 из-за лимита запросов.
        /// </summary>
        public static ReleaseInfo GetLatestVersionFallback()
        {
            string url = VersionUrl + "?t=" + DateTime.Now.Ticks;
            string text = Web.GetString(url, 12000).Trim();
            Match m = Regex.Match(text, "[0-9]+(\\.[0-9]+)*");
            if (!m.Success) throw new ApplicationException("Не удалось определить версию по version.txt: " + Shorten(text));
            ReleaseInfo rel = new ReleaseInfo();
            rel.Tag = m.Value;
            rel.Name = m.Value;
            rel.HtmlUrl = ReleasesPage + "/tag/" + m.Value;
            rel.Body = "Список изменений доступен на странице релиза:\n" + rel.HtmlUrl;
            return rel;
        }

        private static string Shorten(string s)
        {
            if (string.IsNullOrEmpty(s)) return "";
            s = s.Replace("\r", " ").Replace("\n", " ").Trim();
            return s.Length > 80 ? s.Substring(0, 80) + "…" : s;
        }

        private static ReleaseInfo ToRelease(Dictionary<string, object> rel)
        {
            ReleaseInfo r = new ReleaseInfo();
            r.Tag = Str(rel, "tag_name");
            r.Name = Str(rel, "name");
            r.Body = Str(rel, "body");
            r.HtmlUrl = Str(rel, "html_url");
            DateTime published;
            if (DateTime.TryParse(Str(rel, "published_at"), CultureInfo.InvariantCulture, DateTimeStyles.AdjustToUniversal, out published)) r.Published = published.ToLocalTime();
            object assetsObj;
            if (rel.TryGetValue("assets", out assetsObj))
            {
                object[] assets = assetsObj as object[];
                if (assets != null)
                {
                    foreach (object o in assets)
                    {
                        Dictionary<string, object> a = o as Dictionary<string, object>;
                        if (a == null) continue;
                        AssetInfo ai = new AssetInfo();
                        ai.Name = Str(a, "name");
                        ai.Url = Str(a, "browser_download_url");
                        long size;
                        if (long.TryParse(Str(a, "size"), out size)) ai.Size = size;
                        r.Assets.Add(ai);
                    }
                }
            }
            if (r.Tag.Length == 0 && r.Name.Length > 0) r.Tag = r.Name;
            return r;
        }

        private static string Str(Dictionary<string, object> d, string key)
        {
            object v;
            if (d.TryGetValue(key, out v) && v != null) return Convert.ToString(v);
            return "";
        }

        /// <summary>Список последних релизов репозитория (для выбора конкретной версии).</summary>
        public static System.Collections.Generic.List<ReleaseInfo> ListReleases(string repo, int count)
        {
            System.Collections.Generic.List<ReleaseInfo> list = new System.Collections.Generic.List<ReleaseInfo>();
            int perPage = Math.Max(1, Math.Min(50, count));
            string url = string.Format("https://api.github.com/repos/{0}/releases?per_page={1}", repo, perPage);
            string json = Web.GetString(url, 25000);
            JavaScriptSerializer js = new JavaScriptSerializer();
            js.MaxJsonLength = 16 * 1024 * 1024;
            object[] arr = js.DeserializeObject(json) as object[];
            if (arr == null) return list;
            foreach (object item in arr)
            {
                Dictionary<string, object> rel = item as Dictionary<string, object>;
                if (rel == null) continue;
                ReleaseInfo info = ToRelease(rel);
                if (info != null && !string.IsNullOrEmpty(info.Tag)) list.Add(info);
            }
            return list;
        }

        /// <summary>Релиз по конкретному тегу; если API недоступен — синтетический релиз по архиву тега.</summary>
        public static ReleaseInfo ReleaseByTag(string repo, string tag)
        {
            if (string.IsNullOrEmpty(tag)) throw new ApplicationException("Не указан тег версии.");
            try
            {
                string url = string.Format("https://api.github.com/repos/{0}/releases/tags/{1}", repo, Uri.EscapeDataString(tag));
                string json = Web.GetString(url, 20000);
                JavaScriptSerializer js = new JavaScriptSerializer();
                js.MaxJsonLength = 16 * 1024 * 1024;
                Dictionary<string, object> rel = js.DeserializeObject(json) as Dictionary<string, object>;
                if (rel != null) return ToRelease(rel);
            }
            catch (Exception ex)
            {
                Log.Warn("Не удалось получить релиз " + tag + " через API: " + ex.Message);
            }

            ReleaseInfo synthetic = new ReleaseInfo();
            synthetic.Tag = tag;
            synthetic.Name = "zapret-discord-youtube " + tag;
            synthetic.HtmlUrl = string.Format("https://github.com/{0}/releases/tag/{1}", repo, tag);
            synthetic.Body = "Версия " + tag + " (сборка Flowseal).";
            AssetInfo zip = new AssetInfo();
            zip.Name = "zapret-discord-youtube-" + tag + ".zip";
            zip.Url = string.Format("https://github.com/{0}/releases/download/{1}/zapret-discord-youtube-{1}.zip", repo, tag);
            synthetic.Assets.Add(zip);
            return synthetic;
        }

        public static string ZipUrl(ReleaseInfo rel)
        {
            AssetInfo zip = rel.FindZip();
            if (zip != null) return zip.Url;
            return string.Format("https://github.com/{0}/releases/download/{1}/zapret-discord-youtube-{1}.zip", OfficialRepo, rel.Tag);
        }
    }

    public static class Versions
    {
        public static int Compare(string a, string b)
        {
            int[] pa = Parts(a);
            int[] pb = Parts(b);
            int n = Math.Max(pa.Length, pb.Length);
            for (int i = 0; i < n; i++)
            {
                int va = i < pa.Length ? pa[i] : 0;
                int vb = i < pb.Length ? pb[i] : 0;
                if (va != vb) return va < vb ? -1 : 1;
            }
            return 0;
        }

        /// <summary>Тег без префикса v: "v1.10.4" → "1.10.4".</summary>
        public static string NormalizeTag(string tag)
        {
            if (string.IsNullOrEmpty(tag)) return "";
            string t = tag.Trim();
            if (t.StartsWith("v", StringComparison.OrdinalIgnoreCase)) t = t.Substring(1);
            return t;
        }

        private static int[] Parts(string v)
        {
            if (string.IsNullOrEmpty(v)) return new int[0];
            v = v.Trim().TrimStart('v', 'V');
            Match m = Regex.Match(v, "^[0-9]+(\\.[0-9]+)*");
            if (!m.Success) return new int[0];
            string[] raw = m.Value.Split('.');
            int[] parts = new int[raw.Length];
            for (int i = 0; i < raw.Length; i++)
            {
                int x;
                int.TryParse(raw[i], out x);
                parts[i] = x;
            }
            return parts;
        }
    }

    public sealed class UpdateCheckResult
    {
        public bool Ok;
        public string Error = "";
        public string LocalVersion = "";
        public string RemoteVersion = "";
        public bool UpdateAvailable;
        public ReleaseInfo Release;
        public DateTime CheckedAt = DateTime.Now;
        public string Source = "";
        /// <summary>Локальная версия не определена (нет LOCAL_VERSION в service.bat и т.п.).</summary>
        public bool LocalUnknown;
    }

    public static class UpdateChecker
    {
        private static readonly object _lock = new object();
        public static UpdateCheckResult Last;

        public static UpdateCheckResult Check(string localVersion, bool force)
        {
            lock (_lock)
            {
                if (!force && Last != null && (DateTime.Now - Last.CheckedAt).TotalMinutes < 5) return Last;
                UpdateCheckResult res = new UpdateCheckResult();
                res.LocalVersion = localVersion;
                ReleaseInfo rel = null;
                try
                {
                    rel = GitHub.GetLatestRelease(GitHub.OfficialRepo);
                    res.Source = "api";
                }
                catch (Exception apiEx)
                {
                    // Обычная причина: лимит GitHub API (60 запросов в час на IP). Берём version.txt.
                    try
                    {
                        rel = GitHub.GetLatestVersionFallback();
                        res.Source = "version.txt";
                        Log.Warn("GitHub API недоступен (" + apiEx.Message + "), версия получена через version.txt.");
                    }
                    catch (Exception ex2)
                    {
                        res.Ok = false;
                        res.Error = apiEx.Message + (ex2.Message != apiEx.Message ? " / " + ex2.Message : "");
                        Last = res;
                        return res;
                    }
                }
                try
                {
                    res.Release = rel;
                    res.RemoteVersion = rel.Tag;
                    string localCheck = (localVersion ?? "").Trim();
                    res.LocalUnknown = localCheck.Length == 0 || localCheck == "?";
                    res.UpdateAvailable = !res.LocalUnknown && Versions.Compare(rel.Tag, localCheck) > 0;
                    res.Ok = true;
                }
                catch (Exception ex)
                {
                    res.Ok = false;
                    res.Error = ex.Message;
                }
                Last = res;
                return res;
            }
        }
    }

    public sealed class UpdateProgress
    {
        public int Percent;
        public string Stage = "";
        public bool Finished;
        public bool Ok;
        public string Error = "";
        public string Summary = "";
        public bool BackupCreated;
        public string BackupPath = "";
    }

    public sealed class UpdateOptions
    {
        public bool CreateBackup = true;
        public bool RestartAfter = true;
        public bool PreserveUserLists = true;
        public bool UpdateOfficialLists = true;
    }

    /// <summary>Скачивание и установка обновлений zapret с официального репозитория.</summary>
    public sealed class Updater
    {
        private readonly ZapretInstall _install;
        private readonly ZapretOps _ops;
        private readonly Action<UpdateProgress> _report;
        private readonly object _lock = new object();

        public Updater(ZapretInstall install, ZapretOps ops, Action<UpdateProgress> report)
        {
            _install = install;
            _ops = ops;
            _report = report;
        }

        private void Report(int percent, string stage)
        {
            UpdateProgress p = new UpdateProgress();
            p.Percent = percent;
            p.Stage = stage;
            lock (_lock) { _report(p); }
        }

        private void Finish(bool ok, string error, string summary, string backupPath)
        {
            UpdateProgress p = new UpdateProgress();
            p.Finished = true;
            p.Ok = ok;
            p.Percent = 100;
            p.Error = error;
            p.Summary = summary;
            p.BackupPath = backupPath;
            p.BackupCreated = !string.IsNullOrEmpty(backupPath);
            lock (_lock) { _report(p); }
        }

        /// <summary>Полный цикл обновления. Возвращает текст результата.</summary>
        public string UpdateNow(ReleaseInfo rel, UpdateOptions opt)
        {
            string backupPath = "";
            string work = Path.Combine(AppPaths.TempDir, "update-" + rel.Tag);
            try
            {
                if (rel == null) throw new ApplicationException("Нет данных о релизе.");
                if (!_install.IsValid) throw new ApplicationException("Папка zapret не найдена или повреждена.");

                if (Directory.Exists(work)) Directory.Delete(work, true);
                Directory.CreateDirectory(work);
                string zipPath = Path.Combine(work, "update.zip");

                string url = GitHub.ZipUrl(rel);
                Report(2, "Скачивание архива " + rel.Tag + "…");
                Web.Download(url, zipPath, delegate(int percent, long read, long total)
                {
                    int p = percent < 0 ? 10 : (int)(percent * 0.35) + 2;
                    Report(p, string.Format("Скачивание архива: {0}%", percent < 0 ? 0 : percent));
                }, 120000);

                string extractDir = Path.Combine(work, "extract");
                Report(38, "Распаковка архива…");
                ZipFile.ExtractToDirectory(zipPath, extractDir);
                string payload = FindPayloadRoot(extractDir);
                if (payload == null) throw new ApplicationException("В архиве не найдена папка zapret (service.bat + bin\\winws.exe).");
                Log.Info("Обновление: распаковано в " + payload);

                // Состояние до обновления
                RuntimeState before = _ops.GetState();
                string ipsetMode = before.IpsetMode;
                bool hadCheckUpdates = before.CheckUpdates;
                string serviceStrategy = before.ServiceStrategy;
                bool serviceInstalled = before.ServiceInstalled;
                bool wasRunning = before.WinwsRunning;
                string runStrategy = before.MatchedStrategy;

                Report(44, "Остановка bypass…");
                try
                {
                    if (serviceInstalled) Proc.Run("sc.exe", "stop zapret", 25000);
                    Proc.KillAll("winws.exe");
                }
                catch (Exception ex) { Log.Warn("Не удалось остановить bypass перед обновлением: " + ex.Message); }
                Thread.Sleep(600);

                if (opt.CreateBackup)
                {
                    Report(48, "Создание резервной копии…");
                    backupPath = CreateBackup(rel.Tag);
                }

                Report(58, "Установка файлов…");
                int copied = 0;
                int totalFiles = CountPayloadFiles(payload);
                CopyPayload(payload, _install.Root, opt, delegate(string file, int done)
                {
                    copied = done;
                    int p = totalFiles > 0 ? 58 + (int)(done * 34.0 / totalFiles) : 75;
                    Report(p, "Установка файлов: " + Path.GetFileName(file));
                });

                // Восстановление пользовательских состояний
                RestoreState(ipsetMode, hadCheckUpdates, opt);
                Log.Ok(string.Format("Обновление установлено: {0} → {1} (файлов: {2})", before.MatchedStrategy, rel.Tag, copied));

                // Перезапуск
                if (opt.RestartAfter)
                {
                    Report(94, "Перезапуск bypass…");
                    _install.Rescan();
                    StrategyDef target = null;
                    if (serviceInstalled)
                    {
                        target = _install.FindStrategy(serviceStrategy.Length > 0 ? serviceStrategy : runStrategy);
                        if (target == null) target = FindDefaultStrategy();
                        if (target != null)
                        {
                            bool okSvc;
                            _ops.InstallService(target, out okSvc);
                        }
                    }
                    else if (wasRunning)
                    {
                        target = _install.FindStrategy(runStrategy);
                        if (target == null) target = FindDefaultStrategy();
                        if (target != null)
                        {
                            ZapretOps.StartResult sr = _ops.Start(target);
                            if (!sr.Ok) Log.Warn(sr.Message);
                        }
                    }
                }

                try { Directory.Delete(work, true); } catch { }
                // копии при обновлении создаются всегда — держим только две последние
                long freed = Cleanup.PruneBackups(_install.Root, 2, delegate(string m) { Log.Info(m); });
                string extra = freed > 0 ? "\nСтарые копии убраны, освобождено " + Cleanup.FormatSize(freed) : "";
                string summary = string.Format("Обновлено до версии {0}.{1}{2}", rel.Tag, backupPath.Length > 0 ? "\nРезервная копия: " + backupPath : "", extra);
                Finish(true, "", summary, backupPath);
                return summary;
            }
            catch (Exception ex)
            {
                Log.Error("Ошибка обновления", ex);
                Finish(false, ex.Message, "", backupPath);
                return "Ошибка обновления: " + ex.Message;
            }
        }

        private StrategyDef FindDefaultStrategy()
        {
            foreach (StrategyDef s in _install.Strategies)
            {
                if (string.Equals(s.Name, "general", StringComparison.OrdinalIgnoreCase)) return s;
            }
            return _install.Strategies.Count > 0 ? _install.Strategies[0] : null;
        }

        private static string FindPayloadRoot(string dir)
        {
            if (ZapretInstall.LooksValid(dir)) return dir;
            try
            {
                foreach (string d in Directory.GetDirectories(dir))
                {
                    if (ZapretInstall.LooksValid(d)) return d;
                }
            }
            catch { }
            return null;
        }

        private static int CountPayloadFiles(string payload)
        {
            try { return Directory.GetFiles(payload, "*", SearchOption.AllDirectories).Length; }
            catch { return 0; }
        }

        private string CreateBackup(string tag)
        {
            string stamp = DateTime.Now.ToString("yyyyMMdd-HHmmss");
            string parent = Path.GetDirectoryName(_install.Root.TrimEnd('\\'));
            string name = Path.GetFileName(_install.Root.TrimEnd('\\'));
            string dest = Path.Combine(parent, string.Format("{0}_backup_{1}_{2}", name, tag, stamp));
            CopyDirectory(_install.Root, dest, null);
            Log.Info("Резервная копия: " + dest);
            return dest;
        }

        private static void CopyDirectory(string source, string dest, Action<string> progress)
        {
            source = source.TrimEnd('\\');
            Directory.CreateDirectory(dest);
            foreach (string file in Directory.GetFiles(source))
            {
                string name = Path.GetFileName(file);
                if (string.Equals(name, "manager.log", StringComparison.OrdinalIgnoreCase)) continue;
                File.Copy(file, Path.Combine(dest, name), true);
                if (progress != null) progress(name);
            }
            foreach (string dir in Directory.GetDirectories(source))
            {
                string name = Path.GetFileName(dir);
                if (string.Equals(name, "data", StringComparison.OrdinalIgnoreCase)) continue;
                CopyDirectory(dir, Path.Combine(dest, name), progress);
            }
        }

        private void CopyPayload(string payload, string target, UpdateOptions opt, Action<string, int> progress)
        {
            int done = 0;
            string[] files = Directory.GetFiles(payload, "*", SearchOption.AllDirectories);
            foreach (string file in files)
            {
                string rel = file.Substring(payload.TrimEnd('\\').Length).TrimStart('\\');
                done++;
                if (ShouldPreserve(rel, opt)) continue;
                string dest = Path.Combine(target, rel);
                string dir = Path.GetDirectoryName(dest);
                if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir)) Directory.CreateDirectory(dir);
                try
                {
                    File.Copy(file, dest, true);
                }
                catch (Exception ex)
                {
                    Log.Warn("Не удалось скопировать " + rel + ": " + ex.Message);
                }
                if (progress != null) progress(rel, done);
            }
        }

        /// <summary>Файлы, которые обновление не должно затирать.</summary>
        private bool ShouldPreserve(string relPath, UpdateOptions opt)
        {
            string rel = relPath.Replace('\\', '/').ToLowerInvariant();
            string name = Path.GetFileName(rel);
            if (name == "manager.ini" || name == "manager.log") return true;
            if (rel.StartsWith("data/")) return true;
            if (name == Path.GetFileName(System.Reflection.Assembly.GetExecutingAssembly().Location).ToLowerInvariant()) return true;
            if (opt.PreserveUserLists)
            {
                if (name.EndsWith("-user.txt")) return true;
                if (rel == "lists/ipset-all.txt" || rel == "lists/ipset-all.txt.backup") return true;
                if (rel == "utils/targets.txt") return true;
            }
            if (!opt.UpdateOfficialLists)
            {
                if (rel == "lists/list-general.txt" || rel == "lists/list-google.txt" || rel == "lists/list-exclude.txt" || rel == "lists/ipset-exclude.txt") return true;
            }
            if (rel == "utils/game_filter.enabled") return true;
            if (rel == "utils/check_updates.enabled") return true;
            return false;
        }

        private void RestoreState(string ipsetMode, bool hadCheckUpdates, UpdateOptions opt)
        {
            try
            {
                // Пользовательские добавления/удаления в официальных списках переносим на новую версию
                if (opt.UpdateOfficialLists)
                {
                    ListChanges.ApplyAfterUpdate(_install, AppPaths.DataDir, delegate(string message) { Log.Ok(message); });
                }

                // IP-лист: сохраняем пользовательский режим
                if (opt.PreserveUserLists)
                {
                    if (ipsetMode == "none")
                    {
                        File.WriteAllText(_install.IpsetFile, "203.0.113.113/32\r\n", new UTF8Encoding(false));
                    }
                    else if (ipsetMode == "any")
                    {
                        File.WriteAllText(_install.IpsetFile, "", new UTF8Encoding(false));
                    }
                }
                // Флаг авто-проверки обновлений: уважаем выбор пользователя
                if (!hadCheckUpdates)
                {
                    try { if (File.Exists(_install.CheckUpdatesFile)) File.Delete(_install.CheckUpdatesFile); } catch { }
                }
                else if (!File.Exists(_install.CheckUpdatesFile))
                {
                    File.WriteAllText(_install.CheckUpdatesFile, "ENABLED", new UTF8Encoding(false));
                }
            }
            catch (Exception ex) { Log.Warn("Не удалось восстановить состояние после обновления: " + ex.Message); }
        }

        /// <summary>Чистая установка zapret в указанную папку.</summary>
        public static ZapretInstall InstallFresh(ReleaseInfo rel, string parentDir, Action<int, string> progress)
        {
            string work = Path.Combine(AppPaths.TempDir, "install-" + rel.Tag);
            if (Directory.Exists(work)) Directory.Delete(work, true);
            Directory.CreateDirectory(work);
            string zipPath = Path.Combine(work, "zapret.zip");
            if (progress != null) progress(5, "Скачивание архива " + rel.Tag + "…");
            Web.Download(GitHub.ZipUrl(rel), zipPath, delegate(int p, long read, long total)
            {
                if (progress != null) progress(p < 0 ? 20 : 20 + (int)(p * 0.5), "Скачивание архива…");
            }, 120000);
            if (progress != null) progress(75, "Распаковка…");
            string extract = Path.Combine(work, "extract");
            ZipFile.ExtractToDirectory(zipPath, extract);
            string payload = FindPayloadRoot(extract);
            if (payload == null) throw new ApplicationException("В архиве нет папки zapret.");
            string targetName = Path.GetFileName(payload.TrimEnd('\\'));
            string target = Path.Combine(parentDir, targetName);
            if (progress != null) progress(85, "Копирование в " + target + "…");
            if (Directory.Exists(target)) throw new ApplicationException("Папка уже существует: " + target);
            CopyDirectory(payload, target, null);
            if (progress != null) progress(100, "Готово");
            try { Directory.Delete(work, true); } catch { }
            ZapretInstall inst = new ZapretInstall();
            inst.Root = target;
            inst.Rescan();
            return inst;
        }
    }
}
