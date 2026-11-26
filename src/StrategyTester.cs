using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Net.NetworkInformation;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;

namespace ZapretManager
{
    public sealed class TestTarget
    {
        public string Name = "";
        public string Url = "";        // null/пусто для ping-целей
        public string PingHost = "";
        public bool IsPing;

        public string Display
        {
            get { return IsPing ? "ping " + PingHost : Url; }
        }
    }

    public sealed class TestTargetGroup
    {
        public string Name = "";
        public List<TestTarget> Targets = new List<TestTarget>();
    }

    /// <summary>Разбор utils\targets.txt (формат описан в самом файле zapret).</summary>
    public static class TargetsFile
    {
        public static List<TestTargetGroup> Load(string path)
        {
            List<TestTargetGroup> groups = new List<TestTargetGroup>();
            TestTargetGroup current = null;
            try
            {
                if (!File.Exists(path)) return groups;
                foreach (string raw in File.ReadAllLines(path, Encoding.UTF8))
                {
                    string line = raw.Trim();
                    if (line.Length == 0) continue;
                    if (line.StartsWith("###"))
                    {
                        current = new TestTargetGroup();
                        current.Name = line.TrimStart('#').Trim();
                        if (current.Name.Length == 0) current.Name = "Прочее";
                        groups.Add(current);
                        continue;
                    }
                    if (line.StartsWith("#")) continue;
                    Match m = Regex.Match(line, "^([A-Za-z0-9_]+)\\s*=\\s*\"([^\"]*)\"\\s*$");
                    if (!m.Success) continue;
                    if (current == null)
                    {
                        current = new TestTargetGroup();
                        current.Name = "Прочее";
                        groups.Add(current);
                    }
                    string name = m.Groups[1].Value;
                    string value = m.Groups[2].Value.Trim();
                    TestTarget t = new TestTarget();
                    t.Name = name;
                    if (value.StartsWith("PING:", StringComparison.OrdinalIgnoreCase))
                    {
                        t.IsPing = true;
                        t.PingHost = value.Substring(5).Trim();
                    }
                    else
                    {
                        t.Url = value;
                        t.PingHost = Regex.Replace(value, "^https?://", "", RegexOptions.IgnoreCase);
                        int slash = t.PingHost.IndexOf('/');
                        if (slash > 0) t.PingHost = t.PingHost.Substring(0, slash);
                    }
                    if (t.PingHost.Length == 0 && t.Url.Length == 0) continue;
                    current.Targets.Add(t);
                }
            }
            catch (Exception ex)
            {
                Log.Error("Не удалось прочитать targets.txt", ex);
            }
            // группы без целей убираем
            List<TestTargetGroup> clean = new List<TestTargetGroup>();
            foreach (TestTargetGroup g in groups)
            {
                if (g.Targets.Count > 0) clean.Add(g);
            }
            return clean;
        }

        public static TestTargetGroup FindGroup(List<TestTargetGroup> groups, string name)
        {
            foreach (TestTargetGroup g in groups)
            {
                if (string.Equals(g.Name, name, StringComparison.OrdinalIgnoreCase)) return g;
            }
            return null;
        }
    }

    public sealed class GroupResult
    {
        public string Group = "";
        public int Passed;
        public int Total;
        public int Ms;

        public bool Ok { get { return Total > 0 && Passed == Total; } }
        public bool Partial { get { return Passed > 0 && Passed < Total; } }

        public string CellText
        {
            get
            {
                if (Total == 0) return "—";
                if (Passed == 0) return "✗";
                string ms = Ms > 0 ? " " + Ms.ToString() : "";
                return Passed == Total ? "✓" + ms : Passed + "/" + Total + ms;
            }
        }
    }

    public sealed class StrategyTestRow
    {
        public string StrategyName = "";
        public string StrategyFile = "";
        public List<GroupResult> Groups = new List<GroupResult>();
        public string Error = "";

        public int Passed
        {
            get
            {
                int p = 0;
                foreach (GroupResult g in Groups) p += g.Passed;
                return p;
            }
        }

        public int Total
        {
            get
            {
                int t = 0;
                foreach (GroupResult g in Groups) t += g.Total;
                return t;
            }
        }

        public bool AllOk { get { return Total > 0 && Passed == Total; } }
    }

    public sealed class TesterOptions
    {
        public int TimeoutMs = 4000;
        public bool DeepCheck;
        public bool StopService = true;
        public bool RestoreAfter = true;
    }

    /// <summary>Прогон стратегий: запускает winws.exe по очереди и проверяет доступность целей.</summary>
    public sealed class StrategyTester
    {
        private readonly ZapretInstall _install;
        private readonly ZapretOps _ops;

        static StrategyTester()
        {
            // иначе .NET по умолчанию может согласовать TLS 1.0 и проверка ложно провалится
            Web.EnableModernTls();
        }

        public StrategyTester(ZapretInstall install, ZapretOps ops)
        {
            _install = install;
            _ops = ops;
        }

        public static bool CheckHttp(string url, int timeoutMs, bool deep, out int ms, out int status, out string error)
        {
            ms = 0;
            status = 0;
            error = "";
            Stopwatch sw = new Stopwatch();
            try
            {
                HttpWebRequest req = (HttpWebRequest)WebRequest.Create(url);
                req.UserAgent = "ZapretManager/" + AppInfo.Version;
                req.Timeout = timeoutMs;
                req.ReadWriteTimeout = timeoutMs;
                req.Proxy = null;
                req.Method = "GET";
                req.AllowAutoRedirect = true;
                req.AutomaticDecompression = DecompressionMethods.GZip | DecompressionMethods.Deflate;
                req.KeepAlive = false;
                if (deep) req.AddRange(0, 32767);

                sw.Start();
                using (HttpWebResponse resp = (HttpWebResponse)req.GetResponse())
                {
                    status = (int)resp.StatusCode;
                    int want = deep ? 16384 : 512;
                    byte[] buffer = new byte[8192];
                    int total = 0;
                    using (Stream s = resp.GetResponseStream())
                    {
                        int n;
                        while (total < want && (n = s.Read(buffer, 0, buffer.Length)) > 0)
                        {
                            total += n;
                        }
                    }
                    ms = (int)sw.ElapsedMilliseconds;
                    if (deep && total < 16000)
                    {
                        error = "получено всего " + total + " Б";
                        return false;
                    }
                    return true;
                }
            }
            catch (WebException ex)
            {
                ms = (int)sw.ElapsedMilliseconds;
                if (ex.Response != null)
                {
                    try { status = (int)((HttpWebResponse)ex.Response).StatusCode; } catch { }
                    // Сервер ответил (даже 403/404) — значит TLS и HTTP дошли, обход работает.
                    return true;
                }
                error = ex.Status == WebExceptionStatus.Timeout ? "таймаут" : ex.Message;
                if (error.Length > 60) error = error.Substring(0, 60);
                return false;
            }
            catch (Exception ex)
            {
                ms = (int)sw.ElapsedMilliseconds;
                error = ex.Message;
                if (error.Length > 60) error = error.Substring(0, 60);
                return false;
            }
        }

        public static bool CheckPing(string host, int timeoutMs, out int ms, out string error)
        {
            ms = 0;
            error = "";
            try
            {
                using (Ping ping = new Ping())
                {
                    PingReply reply = ping.Send(host, timeoutMs);
                    if (reply == null) { error = "нет ответа"; return false; }
                    ms = (int)reply.RoundtripTime;
                    if (reply.Status == IPStatus.Success) return true;
                    error = reply.Status.ToString();
                    return false;
                }
            }
            catch (Exception ex)
            {
                error = ex.Message;
                if (error.Length > 60) error = error.Substring(0, 60);
                return false;
            }
        }

        /// <summary>Прогон всех стратегий. Возвращает результаты построчно.</summary>
        public List<StrategyTestRow> Run(List<TestTargetGroup> groups, List<StrategyDef> strategies, TesterOptions options,
            Action<int, int, string> progress, Action<StrategyTestRow> rowDone, Func<bool> cancelled)
        {
            List<StrategyTestRow> rows = new List<StrategyTestRow>();
            if (strategies == null || strategies.Count == 0) return rows;
            if (groups == null || groups.Count == 0) return rows;

            RuntimeState before = _ops.GetState();
            ServiceSnapshot svc = ZapretOps.QueryService("zapret");
            bool serviceWasRunning = svc.Installed && svc.Status == "работает";

            try
            {
                if (serviceWasRunning && options.StopService)
                {
                    Log.Info("Тест: останавливаю службу zapret на время прогона.");
                    Proc.Run("sc.exe", "stop zapret", 25000);
                    Thread.Sleep(1200);
                }
                Proc.KillAll("winws.exe");
                Thread.Sleep(300);

                for (int i = 0; i < strategies.Count; i++)
                {
                    if (cancelled != null && cancelled()) break;
                    StrategyDef s = strategies[i];
                    if (progress != null) progress(i + 1, strategies.Count, s.Name);

                    StrategyTestRow row = new StrategyTestRow();
                    row.StrategyName = s.Name;
                    row.StrategyFile = s.File;
                    foreach (TestTargetGroup g in groups)
                    {
                        GroupResult gr = new GroupResult();
                        gr.Group = g.Name;
                        gr.Total = g.Targets.Count;
                        row.Groups.Add(gr);
                    }

                    ZapretOps.StartResult start = _ops.Start(s);
                    if (!start.Ok)
                    {
                        row.Error = start.Message;
                        Log.Warn("Тест: " + s.Name + " — не запустилась: " + start.Message);
                    }
                    else
                    {
                        Thread.Sleep(1200);
                        if (cancelled != null && cancelled()) break;
                        ExecuteChecks(groups, row, options, cancelled);
                    }

                    Proc.KillAll("winws.exe");
                    Thread.Sleep(250);

                    rows.Add(row);
                    if (rowDone != null) rowDone(row);
                }
            }
            finally
            {
                Proc.KillAll("winws.exe");
                Thread.Sleep(300);
                if (options.RestoreAfter)
                {
                    try
                    {
                        if (serviceWasRunning)
                        {
                            Proc.Run("sc.exe", "start zapret", 30000);
                            Log.Info("Тест: служба zapret запущена снова.");
                        }
                        else if (before.WinwsRunning && before.MatchedStrategy.Length > 0)
                        {
                            StrategyDef prev = _install.FindStrategy(before.MatchedStrategy);
                            if (prev != null) _ops.Start(prev);
                        }
                    }
                    catch (Exception ex) { Log.Warn("Не удалось восстановить bypass после теста: " + ex.Message); }
                }
            }
            return rows;
        }

        private static void ExecuteChecks(List<TestTargetGroup> groups, StrategyTestRow row, TesterOptions options, Func<bool> cancelled)
        {
            // Плоский список «цель + группа», проверяем параллельно, чтобы прогон не растягивался на минуты.
            List<KeyValuePair<GroupResult, TestTarget>> flat = new List<KeyValuePair<GroupResult, TestTarget>>();
            foreach (TestTargetGroup g in groups)
            {
                GroupResult gr = null;
                foreach (GroupResult r in row.Groups)
                {
                    if (string.Equals(r.Group, g.Name, StringComparison.OrdinalIgnoreCase)) { gr = r; break; }
                }
                if (gr == null) continue;
                foreach (TestTarget t in g.Targets) flat.Add(new KeyValuePair<GroupResult, TestTarget>(gr, t));
            }

            object gate = new object();
            try
            {
                ParallelOptions po = new ParallelOptions();
                po.MaxDegreeOfParallelism = 6;
                Parallel.ForEach(flat, po, delegate(KeyValuePair<GroupResult, TestTarget> pair)
                {
                    if (cancelled != null && cancelled()) return;
                    GroupResult gr = pair.Key;
                    TestTarget t = pair.Value;
                    bool ok;
                    int ms;
                    string error;
                    if (t.IsPing)
                    {
                        ok = CheckPing(t.PingHost, options.TimeoutMs, out ms, out error);
                    }
                    else
                    {
                        int status;
                        ok = CheckHttp(t.Url, options.TimeoutMs, options.DeepCheck, out ms, out status, out error);
                    }
                    lock (gate)
                    {
                        if (ok) gr.Passed++;
                        if (ms > gr.Ms) gr.Ms = ms;
                    }
                });
            }
            catch (Exception ex)
            {
                Log.Error("Ошибка проверки целей", ex);
            }
        }

        // ---------- сохранение / чтение отчёта ----------

        public static void SaveCsv(string path, List<StrategyTestRow> rows, List<string> groupNames, TesterOptions options)
        {
            try
            {
                StringBuilder sb = new StringBuilder();
                sb.AppendLine("# Zapret Manager — результаты прогона стратегий");
                sb.AppendLine("# " + DateTime.Now.ToString("dd.MM.yyyy HH:mm:ss") + "; таймаут " + options.TimeoutMs + " мс" + (options.DeepCheck ? "; глубокая проверка 16 КБ" : ""));
                sb.AppendLine("groups\t" + string.Join("\t", groupNames.ToArray()));
                foreach (StrategyTestRow r in rows)
                {
                    sb.Append(r.StrategyName);
                    foreach (string g in groupNames)
                    {
                        GroupResult gr = null;
                        foreach (GroupResult x in r.Groups)
                        {
                            if (string.Equals(x.Group, g, StringComparison.OrdinalIgnoreCase)) { gr = x; break; }
                        }
                        sb.Append('\t');
                        if (gr == null) sb.Append("-");
                        else sb.Append(gr.Passed + "/" + gr.Total + ":" + gr.Ms);
                    }
                    sb.Append('\t');
                    sb.Append(r.Passed + "/" + r.Total);
                    if (r.Error.Length > 0) sb.Append("\t!" + r.Error.Replace("\t", " ").Replace("\n", " "));
                    sb.AppendLine();
                }
                string dir = Path.GetDirectoryName(path);
                if (!string.IsNullOrEmpty(dir) && !Directory.Exists(dir)) Directory.CreateDirectory(dir);
                File.WriteAllText(path, sb.ToString(), new UTF8Encoding(false));
            }
            catch (Exception ex) { Log.Error("Не удалось сохранить отчёт теста", ex); }
        }

        public sealed class LoadedReport
        {
            public List<StrategyTestRow> Rows = new List<StrategyTestRow>();
            public List<string> Groups = new List<string>();
            public string Header = "";
        }

        public static LoadedReport LoadCsv(string path)
        {
            LoadedReport rep = new LoadedReport();
            try
            {
                if (!File.Exists(path)) return rep;
                string[] lines = File.ReadAllLines(path, Encoding.UTF8);
                bool groupsRead = false;
                foreach (string raw in lines)
                {
                    if (raw.Length == 0) continue;
                    if (raw.StartsWith("groups\t"))
                    {
                        string[] parts = raw.Split('\t');
                        for (int i = 1; i < parts.Length; i++) rep.Groups.Add(parts[i]);
                        groupsRead = true;
                        continue;
                    }
                    if (raw.StartsWith("#"))
                    {
                        if (rep.Header.Length == 0 && raw.StartsWith("# ") && raw.IndexOf("Zapret") < 0) rep.Header = raw.TrimStart('#', ' ');
                        continue;
                    }
                    if (!groupsRead) continue;
                    string[] cells = raw.Split('\t');
                    if (cells.Length < 2) continue;
                    StrategyTestRow row = new StrategyTestRow();
                    row.StrategyName = cells[0];
                    for (int i = 1; i <= rep.Groups.Count && i < cells.Length; i++)
                    {
                        GroupResult gr = new GroupResult();
                        gr.Group = rep.Groups[i - 1];
                        string cell = cells[i];
                        if (cell.StartsWith("-")) { gr.Total = 0; }
                        else
                        {
                            int slash = cell.IndexOf('/');
                            int colon = cell.IndexOf(':');
                            if (slash > 0 && colon > slash)
                            {
                                int passed;
                                int total;
                                int ms;
                                int.TryParse(cell.Substring(0, slash), out passed);
                                int.TryParse(cell.Substring(slash + 1, colon - slash - 1), out total);
                                int.TryParse(cell.Substring(colon + 1), out ms);
                                gr.Passed = passed;
                                gr.Total = total;
                                gr.Ms = ms;
                            }
                        }
                        row.Groups.Add(gr);
                    }
                    int last = cells.Length - 1;
                    if (cells[last].StartsWith("!")) row.Error = cells[last].Substring(1);
                    rep.Rows.Add(row);
                }
            }
            catch (Exception ex) { Log.Error("Не удалось прочитать отчёт теста", ex); }
            return rep;
        }
    }
    /// <summary>Разбор отчёта официального скрипта utils\test zapret.ps1 (секция ANALYTICS).</summary>
    public sealed class OfficialTestReport
    {
        public string FilePath = "";
        public DateTime Created;
        public string Best = "";
        public List<OfficialTestRow> Rows = new List<OfficialTestRow>();

        public int TotalOk
        {
            get
            {
                int n = 0;
                foreach (OfficialTestRow r in Rows) n += r.Ok;
                return n;
            }
        }

        public static List<string> ListReports(string resultsDir)
        {
            List<string> files = new List<string>();
            try
            {
                if (!Directory.Exists(resultsDir)) return files;
                string[] found = Directory.GetFiles(resultsDir, "test_results_*.txt");
                Array.Sort(found);
                Array.Reverse(found);
                files.AddRange(found);
            }
            catch { }
            return files;
        }

        public static OfficialTestReport Parse(string path)
        {
            OfficialTestReport rep = new OfficialTestReport();
            rep.FilePath = path;
            try
            {
                rep.Created = File.GetLastWriteTime(path);
                bool inAnalytics = false;
                foreach (string raw in File.ReadAllLines(path, Encoding.UTF8))
                {
                    string line = raw.Trim();
                    if (line.StartsWith("=== ANALYTICS", StringComparison.OrdinalIgnoreCase)) { inAnalytics = true; continue; }
                    if (line.StartsWith("Best strategy:", StringComparison.OrdinalIgnoreCase))
                    {
                        rep.Best = line.Substring("Best strategy:".Length).Trim();
                        continue;
                    }
                    if (!inAnalytics || line.Length == 0) continue;
                    Match m = Regex.Match(line,
                        "^(?<name>.+?)\\s*:\\s*OK:\\s*(?<ok>\\d+),\\s*FAIL:\\s*(?<fail>\\d+),\\s*UNSUP:\\s*(?<unsup>\\d+),\\s*BLOCKED:\\s*(?<blocked>\\d+)\\s*$");
                    if (!m.Success) continue;
                    OfficialTestRow row = new OfficialTestRow();
                    row.Name = m.Groups["name"].Value.Trim();
                    row.Ok = int.Parse(m.Groups["ok"].Value);
                    row.Fail = int.Parse(m.Groups["fail"].Value);
                    row.Unsup = int.Parse(m.Groups["unsup"].Value);
                    row.Blocked = int.Parse(m.Groups["blocked"].Value);
                    rep.Rows.Add(row);
                }
            }
            catch (Exception ex) { Log.Error("Не удалось разобрать отчёт официального теста", ex); }
            rep.Rows.Sort(delegate(OfficialTestRow a, OfficialTestRow b) { return b.Ok.CompareTo(a.Ok); });
            return rep;
        }
    }

    public sealed class OfficialTestRow
    {
        public string Name = "";
        public int Ok;
        public int Fail;
        public int Unsup;
        public int Blocked;

        public int Total { get { return Ok + Fail + Unsup + Blocked; } }

        public int Percent
        {
            get { return Total == 0 ? 0 : (int)Math.Round(Ok * 100.0 / Total); }
        }

        public string Details
        {
            get
            {
                return string.Format("OK {0} · FAIL {1} · BLOCKED {2} · {3}%", Ok, Fail, Blocked, Percent);
            }
        }
    }
}