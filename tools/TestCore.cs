using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Threading;
using ZapretManager;

internal static class TestCore
{
    private static StringBuilder _sb = new StringBuilder();

    private static void W(string line)
    {
        Console.WriteLine(line);
        _sb.AppendLine(line);
    }

    private static int Main(string[] args)
    {
        Console.OutputEncoding = Encoding.UTF8;
        string action = args.Length > 0 ? args[0] : "info";
        string argument = args.Length > 1 ? args[1] : "";
        string logPath = args.Length > 2 ? args[2] : null;
        string rootArg = args.Length > 3 ? args[3] : null;

        ZapretInstall inst = ZapretInstall.Detect(rootArg);
        ZapretOps ops = new ZapretOps(inst);
        W("root       = " + inst.Root);
        W("valid      = " + inst.IsValid);
        W("version    = " + inst.Version);
        W("admin      = " + Native.IsAdmin());
        W("action     = " + action + " " + argument);

        try
        {
            switch (action)
            {
                case "info":
                    W("strategies = " + inst.StrategyCount);
                    foreach (StrategyDef s in inst.Strategies)
                        W(string.Format("  [{0}] {1} | {2} | {3}", s.Order, s.Name, s.ShortName, s.Description));
                    DumpState(ops);
                    break;

                case "start":
                {
                    StrategyDef s = inst.FindStrategy(argument);
                    if (s == null) { W("ERROR: strategy not found: " + argument); break; }
                    ServiceSnapshot svc = ZapretOps.QueryService("zapret");
                    if (svc.Installed && svc.Status == "работает")
                    {
                        W("stopping service zapret first…");
                        ProcResult r = Proc.Run("sc.exe", "stop zapret", 30000);
                        W("  sc stop -> " + r.ExitCode + " " + r.All);
                        Thread.Sleep(2000);
                    }
                    ZapretOps.StartResult res = ops.Start(s);
                    W("start -> ok=" + res.Ok + " pid=" + res.Pid + " msg=" + res.Message);
                    Thread.Sleep(3000);
                    List<Proc.ProcInfo> procs = Proc.List("winws.exe");
                    W("winws processes after start: " + procs.Count);
                    foreach (Proc.ProcInfo pi in procs) W("  pid " + pi.Pid + " started " + pi.Started);
                    break;
                }

                case "stop":
                {
                    string msg = ops.Stop();
                    W("stop -> " + msg);
                    Thread.Sleep(1500);
                    W("winws processes after stop: " + Proc.List("winws.exe").Count);
                    break;
                }

                case "svc-install":
                {
                    StrategyDef s = inst.FindStrategy(argument);
                    if (s == null) { W("ERROR: strategy not found: " + argument); break; }
                    bool ok;
                    string msg = ops.InstallService(s, out ok);
                    W("install-service -> ok=" + ok + " msg=" + msg);
                    Thread.Sleep(2000);
                    ServiceSnapshot svc = ZapretOps.QueryService("zapret");
                    W("service installed=" + svc.Installed + " status=" + svc.Status + " strategy=" + svc.Strategy);
                    W("image path: " + svc.ImagePath);
                    break;
                }

                case "svc-remove":
                {
                    string msg = ops.RemoveService();
                    W("remove-service -> " + msg);
                    Thread.Sleep(1200);
                    ServiceSnapshot svc = ZapretOps.QueryService("zapret");
                    W("service installed=" + svc.Installed);
                    break;
                }

                case "svc-create-raw":
                {
                    // восстановление исходной службы по сохранённому ImagePath
                    string imagePath = File.ReadAllText(argument, Encoding.UTF8).Trim();
                    Proc.Run("sc.exe", "stop zapret", 20000);
                    Thread.Sleep(500);
                    Proc.Run("sc.exe", "delete zapret", 20000);
                    Thread.Sleep(500);
                    string createArgs = "create zapret binPath= \"" + imagePath.Replace("\"", "\\\"") + "\" DisplayName= \"zapret\" start= auto";
                    ProcResult cr = Proc.Run("sc.exe", createArgs, 20000);
                    W("sc create -> " + cr.ExitCode + " " + cr.All);
                    Proc.Run("sc.exe", "description zapret \"Zapret DPI bypass software\"", 10000);
                    ProcResult sr = Proc.Run("sc.exe", "start zapret", 30000);
                    W("sc start -> " + sr.ExitCode + " " + sr.All);
                    Thread.Sleep(2000);
                    ServiceSnapshot svc = ZapretOps.QueryService("zapret");
                    W("restored: installed=" + svc.Installed + " status=" + svc.Status);
                    W("winws: " + Proc.List("winws.exe").Count);
                    break;
                }

                case "ipsets":
                {
                    W("mode before: " + ops.GetIpsetMode());
                    W(ops.SetIpsetMode("loaded"));
                    W("mode after loaded: " + ops.GetIpsetMode() + " lines=" + ops.CountListLines(inst.IpsetFile) + " backup=" + ops.CountListLines(inst.IpsetBackup));
                    W(ops.SetIpsetMode("none"));
                    W("mode after none: " + ops.GetIpsetMode() + " lines=" + ops.CountListLines(inst.IpsetFile));
                    W(ops.SetIpsetMode("loaded"));
                    W("mode restored: " + ops.GetIpsetMode() + " lines=" + ops.CountListLines(inst.IpsetFile));
                    break;
                }

                case "update":
                {
                    ReleaseInfo rel = GitHub.GetLatestRelease(GitHub.OfficialRepo);
                    W("latest release: " + rel.Tag + " published " + rel.Published.ToString("u"));
                    W("zip url: " + GitHub.ZipUrl(rel));
                    UpdateOptions opt = new UpdateOptions();
                    opt.CreateBackup = true;
                    opt.RestartAfter = true;
                    opt.PreserveUserLists = true;
                    Updater upd = new Updater(inst, ops, delegate(UpdateProgress p)
                    {
                        if (p.Finished) W(string.Format("  done ok={0} err={1} summary={2} backup={3}", p.Ok, p.Error, p.Summary.Replace("\n", " "), p.BackupPath));
                        else if (p.Percent % 10 < 3) W(string.Format("  {0}% {1}", p.Percent, p.Stage));
                    });
                    string result = upd.UpdateNow(rel, opt);
                    W("update result: " + result);
                    DumpState(ops);
                    break;
                }

                case "check":
                {
                    UpdateCheckResult res = UpdateChecker.Check(inst.Version, true);
                    W("check ok=" + res.Ok + " local=" + res.LocalVersion + " remote=" + res.RemoteVersion +
                      " available=" + res.UpdateAvailable + " err=" + res.Error);
                    if (res.Release != null) W("release: " + res.Release.Tag + " html=" + res.Release.HtmlUrl + " assets=" + res.Release.Assets.Count);
                    break;
                }

                case "diag":
                {
                    List<CheckResult> results = Diagnostics.Run(inst, ops, AppConfig.Load(Path.Combine(inst.Root, "data", "manager.ini")), delegate(string step) { Console.Write("."); });
                    Console.WriteLine();
                    foreach (CheckResult r in results)
                        W(string.Format("[{0}] {1}: {2}{3}", r.SeverityText, r.Name, r.Message, r.Hint.Length > 0 ? " | " + r.Hint : ""));
                    break;
                }

                case "netsh":
                {
                    ProcResult r = Proc.Run("netsh.exe", "interface tcp show global", 8000);
                    W("netsh exit=" + r.ExitCode + " outLen=" + r.StdOut.Length + " errLen=" + r.StdErr.Length);
                    W("netsh out: " + r.StdOut.Replace("\r", "").Replace("\n", " | "));
                    W("netsh err: " + r.StdErr);
                    ProcResult r2 = Proc.Run("cmd.exe", "/c chcp 437 >nul & netsh interface tcp show global", 8000);
                    W("cmd-netsh exit=" + r2.ExitCode + " outLen=" + r2.StdOut.Length);
                    W("cmd-netsh has timestamps=" + (r2.All.ToLowerInvariant().Contains("timestamps")) + " has enabled=" + (r2.All.ToLowerInvariant().Contains("enabled")));
                    break;
                }

                case "lists":
                {
                    string dataDir = Path.Combine(inst.Root, "data");
                    Directory.CreateDirectory(dataDir);
                    string file = Path.Combine(inst.ListsDir, "list-general.txt");
                    W("--- исходный файл: " + new FileInfo(file).Length + " байт");
                    ListChanges ch = ListChanges.Load(file, dataDir);
                    W("baseline=" + ch.BaselineEntries.Count + " added=" + ch.Added.Count + " removed=" + ch.Removed.Count);
                    string text = File.ReadAllText(file, Encoding.UTF8);
                    string[] lines = text.Replace("\r\n", "\n").Split('\n');
                    List<string> off; List<string> block;
                    ListChanges.SplitBlock(lines, out off, out block);
                    // пользователь правит: добавляет 2 строки и удаляет одну официальную
                    List<string> edited = new List<string>(off);
                    edited.Insert(0, "mydomain-test.example");
                    string victim = null;
                    for (int i = edited.Count - 1; i >= 0; i--)
                    {
                        string t = (edited[i] ?? "").Trim();
                        if (t.Length > 0 && !t.StartsWith("#")) { victim = t; edited.RemoveAt(i); break; }
                    }
                    edited.Add("second-test.example");
                    string saved = ch.SaveFromEditor(string.Join("\r\n", edited.ToArray()));
                    W("удалена официальная строка: " + victim);
                    W("после сохранения: added=" + ch.Added.Count + " removed=" + ch.Removed.Count);
                    W("блок в файле: " + (saved.Contains(">>> Zapret Manager") ? "есть" : "НЕТ"));
                    W("добавления в файле: " + string.Join(", ", ch.Added.ToArray()));
                    W("удалённые в файле: " + string.Join(", ", ch.Removed.ToArray()));
                    string chFile = ListChanges.ChangesPath(dataDir, "list-general.txt");
                    W("файл изменений: " + (File.Exists(chFile) ? File.ReadAllText(chFile).Replace("\r\n", " | ") : "нет"));
                    // имитация обновления: новая официальная версия +2 строки
                    List<string> newOfficial = new List<string>();
                    foreach (string l in off) if ((l ?? "").Trim().Length > 0) newOfficial.Add(l.Trim());
                    newOfficial.Add("developer-new-1.example");
                    newOfficial.Add("developer-new-2.example");
                    File.WriteAllText(file, string.Join("\r\n", newOfficial.ToArray()) + "\r\n", Encoding.UTF8);
                    ListChanges.ApplyAfterUpdate(inst, dataDir, delegate(string m) { W("  update: " + m); });
                    ListChanges after = ListChanges.Load(file, dataDir);
                    W("после обновления: baseline=" + after.BaselineEntries.Count + " added=" + after.Added.Count + " removed=" + after.Removed.Count + " news=" + after.News.Count);
                    W("news: " + string.Join(", ", after.News.ToArray()));
                    W("файл содержит mydomain-test.example: " + File.ReadAllText(file).Contains("mydomain-test.example"));
                    W("файл содержит developer-new-1.example: " + File.ReadAllText(file).Contains("developer-new-1.example"));
                    W("файл содержит удалённую строку: " + (victim != null && File.ReadAllText(file).Contains(victim)));
                    break;
                }

                case "autostart":
                {
                    W("task before: [" + Autostart.Query().Replace("\r\n", " | ") + "]");
                    W("enable -> " + Autostart.Enable(System.Reflection.Assembly.GetExecutingAssembly().Location, true));
                    string q = Autostart.Query();
                    W("task after enable: " + (q.Length > 0 ? "ЕСТЬ" : "НЕТ"));
                    foreach (string line in q.Split('\n'))
                    {
                        string t = line.Trim();
                        if (t.StartsWith("TaskName") || t.StartsWith("Task To Run") || t.StartsWith("Run As User") || t.StartsWith("Scheduled Task State") || t.StartsWith("Имя задачи") || t.StartsWith("Выполняемая задача") || t.StartsWith("Запуск от имени")) W("   " + t);
                    }
                    W("disable -> " + Autostart.Disable());
                    W("task after disable: " + (Autostart.Query().Length > 0 ? "ЕСТЬ" : "НЕТ"));
                    break;
                }

                case "tginstall":
                {
                    AppConfig cfg = AppConfig.Load(Path.Combine(inst.Root, "data", "manager.ini"));
                    ReleaseInfo rel = TgProxy.CheckLatest();
                    W("install " + rel.Tag + " -> " + argument);
                    string msg;
                    bool ok;
                    msg = TgProxy.Install(cfg, rel.Tag, argument, delegate(int percent, string stage) { Console.Write("."); }, out ok);
                    Console.WriteLine();
                    W("result ok=" + ok + " msg=" + msg);
                    string exe = Path.Combine(argument, "TgWsProxy_windows.exe");
                    W("exe exists=" + File.Exists(exe) + (File.Exists(exe) ? " size=" + new FileInfo(exe).Length : ""));
                    W("version.txt=" + (File.Exists(Path.Combine(argument, "version.txt")) ? File.ReadAllText(Path.Combine(argument, "version.txt")) : "нет"));
                    TgProxyState st2 = TgProxy.GetState(cfg);
                    W("state after: installed=" + st2.Installed + " dir=" + st2.Dir + " version=" + st2.Version + " running=" + st2.Running);
                    break;
                }

                case "tg":
                {
                    AppConfig cfg = AppConfig.Load(Path.Combine(inst.Root, "data", "manager.ini"));
                    TgProxyState st = TgProxy.GetState(cfg);
                    W("installed=" + st.Installed + " running=" + st.Running + " instances=" + st.InstanceCount);
                    W("exe=" + st.ExePath);
                    W("dir=" + st.Dir);
                    W("version=" + st.Version);
                    W("port=" + st.Port + " host=" + st.Host + " listening=" + st.PortListening);
                    W("secret=" + (st.Secret.Length > 0 ? st.Secret.Substring(0, Math.Min(8, st.Secret.Length)) + "…" : "нет"));
                    W("link=" + st.Link);
                    W("pid=" + st.Pid + " started=" + st.Started);
                    ReleaseInfo rel = TgProxy.CheckLatest();
                    W("latest=" + rel.Tag + " url=" + TgProxy.DownloadUrl(rel.Tag));
                    W("update available=" + TgProxy.UpdateAvailable(st, rel));
                    // проверка скачивания без установки
                    string tmp = Path.Combine(Path.GetTempPath(), "tgws-test-" + rel.Tag + ".exe");
                    if (File.Exists(tmp)) File.Delete(tmp);
                    System.Diagnostics.Stopwatch sw = System.Diagnostics.Stopwatch.StartNew();
                    Web.Download(TgProxy.DownloadUrl(rel.Tag), tmp, null, 300000);
                    sw.Stop();
                    FileInfo fi = new FileInfo(tmp);
                    W(string.Format("download: {0} bytes за {1:0.0} с", fi.Length, sw.Elapsed.TotalSeconds));
                    using (FileStream fs = File.OpenRead(tmp)) { W("signature: " + (char)fs.ReadByte() + (char)fs.ReadByte()); }
                    W("sha256: " + ZapretOps.Sha256File(tmp));
                    File.Delete(tmp);
                    break;
                }

                case "httpv":
                {
                    string url = "https://discord.com";
                    W("SecurityProtocol before: " + System.Net.ServicePointManager.SecurityProtocol);
                    System.Net.ServicePointManager.SecurityProtocol = System.Net.SecurityProtocolType.Tls12;
                    W("SecurityProtocol now: " + System.Net.ServicePointManager.SecurityProtocol);
                    for (int variant = 0; variant <= 6; variant++)
                    {
                        try
                        {
                            System.Net.HttpWebRequest req = (System.Net.HttpWebRequest)System.Net.WebRequest.Create(url);
                            req.Method = "GET";
                            req.Timeout = 8000;
                            if (variant >= 1) req.Proxy = null;
                            if (variant >= 2) req.ReadWriteTimeout = 4000;
                            if (variant >= 3) req.AutomaticDecompression = System.Net.DecompressionMethods.GZip | System.Net.DecompressionMethods.Deflate;
                            if (variant >= 4) req.UserAgent = "ZapretManager/1.0";
                            if (variant >= 5) req.KeepAlive = false;
                            if (variant >= 6) req.AllowAutoRedirect = true;
                            using (System.Net.HttpWebResponse resp = (System.Net.HttpWebResponse)req.GetResponse())
                            {
                                W("variant " + variant + " -> OK " + (int)resp.StatusCode);
                            }
                        }
                        catch (Exception ex)
                        {
                            W("variant " + variant + " -> FAIL " + ex.Message);
                        }
                    }
                    break;
                }

                case "http":
                {
                    string[] urls = new string[] { "https://discord.com", "https://www.google.com", "https://cdn.discordapp.com", "https://www.youtube.com" };
                    foreach (string u in urls)
                    {
                        int ms; int st; string err;
                        bool ok = StrategyTester.CheckHttp(u, 4000, false, out ms, out st, out err);
                        W(u + " -> ok=" + ok + " ms=" + ms + " status=" + st + " err=" + err);
                    }
                    break;
                }

                case "test2":
                {
                    List<TestTargetGroup> groups = TargetsFile.Load(Path.Combine(inst.UtilsDir, "targets.txt"));
                    W("группы целей: " + groups.Count);
                    foreach (TestTargetGroup g in groups) W("  " + g.Name + " -> " + g.Targets.Count);
                    List<TestTargetGroup> use = new List<TestTargetGroup>();
                    foreach (TestTargetGroup g in groups) if (g.Name.ToLowerInvariant().Contains("discord")) use.Add(g);
                    if (use.Count == 0 && groups.Count > 0) use.Add(groups[0]);
                    List<StrategyDef> defs = new List<StrategyDef>();
                    defs.Add(inst.FindStrategy("general"));
                    defs.Add(inst.FindStrategy("general (ALT)"));
                    for (int i = defs.Count - 1; i >= 0; i--) if (defs[i] == null) defs.RemoveAt(i);
                    W("стратегий к тесту: " + defs.Count);
                    StrategyTester tester = new StrategyTester(inst, ops);
                    TesterOptions opt = new TesterOptions();
                    opt.TimeoutMs = 4000;
                    List<StrategyTestRow> rows = tester.Run(use, defs, opt,
                        delegate(int d, int t, string n) { W("  probe " + d + "/" + t + " " + n); },
                        delegate(StrategyTestRow r) { W("  row: " + r.StrategyName + " " + r.Passed + "/" + r.Total + " " + r.Error); },
                        delegate() { return false; });
                    foreach (StrategyTestRow r in rows)
                    {
                        W(r.StrategyName + " -> " + r.Passed + "/" + r.Total + (r.Error.Length > 0 ? " err=" + r.Error : ""));
                        foreach (GroupResult gr in r.Groups) W("   " + gr.Group + ": " + gr.CellText);
                    }
                    string report = Path.Combine(inst.Root, "data", "test-results.tsv");
                    List<string> gn = new List<string>();
                    foreach (TestTargetGroup g in use) gn.Add(g.Name);
                    StrategyTester.SaveCsv(report, rows, gn, opt);
                    W("отчёт: " + report);
                    break;
                }

                case "hosts":
                {
                    ZapretOps.HostsStatus st = ops.CheckHosts();
                    W("hosts downloaded=" + st.Downloaded + " remoteLines=" + st.RemoteLines + " missing=" + st.MissingLines + " err=" + st.Error);
                    break;
                }
            }
        }
        catch (Exception ex)
        {
            W("EXCEPTION: " + ex);
        }

        if (!string.IsNullOrEmpty(logPath)) File.WriteAllText(logPath, _sb.ToString(), Encoding.UTF8);
        return 0;
    }

    private static void DumpState(ZapretOps ops)
    {
        RuntimeState st = ops.GetState();
        W("state: winws=" + st.WinwsRunning + " pid=" + st.WinwsPid + " matched=" + st.MatchedStrategy);
        W("       service=" + st.ServiceInstalled + "/" + st.ServiceStatus + "/" + st.ServiceStrategy);
        W("       ipset=" + st.IpsetMode + " lines=" + st.IpsetLines + " gamefilter=" + st.GameFilter.ModeText);
    }
}
