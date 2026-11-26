using System;
using System.Diagnostics;
using System.Drawing;
using System.IO;
using System.Text;
using System.Threading;
using System.Windows.Forms;

namespace ZapretManager
{
    internal static class Program
    {
        private static Mutex _mutex;

        [STAThread]
        private static void Main(string[] args)
        {
            // Применение обновления: новый exe ждёт выхода старого, копирует себя на место
            // старого и запускает его. Никакого cmd.exe и кодировок — пути остаются Unicode.
            if (HasFlag(args, "--apply-update"))
            {
                SelfUpdate.RunApplyUpdate();
                return;
            }

            Web.EnableModernTls();
            Theme.InitFonts();
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
            Application.ThreadException += delegate(object s, ThreadExceptionEventArgs e) { ReportCrash(e.Exception); };
            AppDomain.CurrentDomain.UnhandledException += delegate(object s, UnhandledExceptionEventArgs e) { ReportCrash(e.ExceptionObject as Exception); };

            string rootArg = FindArgValue(args, "--root");
            AppInfo.ShotPath = FindArgValue(args, "--shot");
            AppInfo.ShotPage = FindArgValue(args, "--shot-page");
            AppInfo.ShotList = FindArgValue(args, "--shot-list");
            AppInfo.ShotSite = FindArgValue(args, "--shot-site");
            int shotScroll;
            if (int.TryParse(FindArgValue(args, "--shot-scroll"), out shotScroll)) AppInfo.ShotScroll = shotScroll;
            bool shotMode = !string.IsNullOrEmpty(AppInfo.ShotPath) || !string.IsNullOrEmpty(FindArgValue(args, "--menu-shot"));
            bool selfUpdateMode = HasFlag(args, "--self-update");
            bool tgPrepareMode = HasFlag(args, "--tg-prepare");
            bool guardOnce = HasFlag(args, "--guard-once");
            bool tgStartMode = HasFlag(args, "--tg-start");
            bool tgStopMode = HasFlag(args, "--tg-stop");
            string msgBoxShot = FindArgValue(args, "--msgbox-shot");
            bool checkSelfMode = HasFlag(args, "--check-self");
            bool restoreUacMode = HasFlag(args, "--restore-uac");
            bool cleanupMode = HasFlag(args, "--cleanup");
            string menuShotPath = FindArgValue(args, "--menu-shot");
            bool menuCursorProbe = HasFlag(args, "--menu-probe");
            string appWatch = FindArgValue(args, "--app-watch");
            AppInfo.TrayMode = HasFlag(args, "--tray");
            AppInfo.AutostartMode = HasFlag(args, "--autostart");
            if (AppInfo.AutostartMode) AppInfo.TrayMode = true;
            bool limited = HasFlag(args, "--limited") || shotMode;
            bool elevated = HasFlag(args, "--elevated");

            if (!Native.IsAdmin() && !limited && !elevated && !shotMode)
            {
                // Пытаемся сразу перезапуститься с правами администратора: управление службами и winws.exe требуют elevation.
                try
                {
                    ProcessStartInfo psi = new ProcessStartInfo(Application.ExecutablePath, BuildElevatedArgs(args));
                    psi.UseShellExecute = true;
                    psi.Verb = "runas";
                    psi.WorkingDirectory = Path.GetDirectoryName(Application.ExecutablePath);
                    Process.Start(psi);
                    return;
                }
                catch
                {
                    // Пользователь отказался от UAC — работаем в ограниченном режиме (просмотр и настройка списков).
                }
            }

            try { _mutex = new Mutex(true, "ZapretManager_SingleInstance", out _mutexCreated); }
            catch { _mutexCreated = true; }
            if (!_mutexCreated && !shotMode)
            {
                ActivateExistingWindow();
                return;
            }
            try
            {
                AppInfo.ActivateEvent = new EventWaitHandle(false, EventResetMode.AutoReset, "ZapretManager_Activate");
            }
            catch { }
            Log.FilePath = Path.Combine(AppPaths.DataDir, "manager.log");
            try
            {
                long cleaned = Cleanup.CleanTemp(7);
                if (cleaned > 0) Log.Info("Уборка временных файлов обновлений: " + Cleanup.FormatSize(cleaned));
            }
            catch { }

            Log.Info("=== Zapret Manager " + AppInfo.Version + " запуск ===");
            if (selfUpdateMode)
            {
                RunSelfUpdateMode();
                return;
            }
            if (tgStartMode || tgStopMode)
            {
                AppConfig tgConfig = AppConfig.Load(AppConfig.DefaultPath());
                string tgResult;
                if (tgStopMode)
                {
                    tgResult = TgProxy.Stop();
                }
                else
                {
                    bool tgOk;
                    tgResult = TgProxy.Start(tgConfig, out tgOk);
                }
                Log.Info("Команда TG WS Proxy: " + tgResult);
                return;
            }
            if (guardOnce)
            {
                AppConfig guardConfig = AppConfig.Load(AppConfig.DefaultPath());
                ZapretInstall guardInstall = ZapretInstall.Detect(string.IsNullOrEmpty(rootArg) ? guardConfig.Get("Root", "") : rootArg);
                SiteGuard.AllowAllProcesses = true;
                SiteGuard.DebugFirst = 6;
                string guardError;
                int guardScanned;
                System.Collections.Generic.List<string> guardHosts = SiteGuard.NewBrowserHosts(out guardError, out guardScanned);
                Log.Info("Сторож: просканировано событий " + guardScanned + ", доменов " + guardHosts.Count + (guardError.Length > 0 ? ", ошибка: " + guardError : ""));
                if (guardInstall.IsValid && guardHosts.Count > 0)
                {
                    bool guardAutoAdd = guardConfig.GetBool("SiteGuardAutoAdd", false);
                    System.Collections.Generic.List<string> guardAdded = SiteGuard.Process(guardInstall, guardConfig, guardHosts, guardAutoAdd,
                        delegate(string guardMessage) { Log.Info(guardMessage); });
                    Log.Info("Сторож: добавлено доменов " + guardAdded.Count + (guardAutoAdd ? "." : " (без авто-добавления)."));
                }
                return;
            }
            if (menuCursorProbe)
            {
                Theme.Apply("dark");
                MenuPreview.RunAtCursor("");
                return;
            }
            if (!string.IsNullOrEmpty(menuShotPath))
            {
                Theme.Apply("dark");
                MenuPreview.Run(menuShotPath);
                return;
            }
            if (cleanupMode)
            {
                AppConfig cleanConfig = AppConfig.Load(AppConfig.DefaultPath());
                ZapretInstall cleanInstall = ZapretInstall.Detect(string.IsNullOrEmpty(rootArg) ? cleanConfig.Get("Root", "") : rootArg);
                System.Collections.Generic.List<DirectoryInfo> copies = Cleanup.Backups(cleanInstall == null ? "" : cleanInstall.Root);
                long total = 0;
                foreach (DirectoryInfo d in copies) total += Cleanup.SizeOf(d.FullName);
                Log.Info(string.Format("Уборка: копий {0}, занято {1}", copies.Count, Cleanup.FormatSize(total)));
                long freed = Cleanup.PruneBackups(cleanInstall == null ? "" : cleanInstall.Root, 1, delegate(string m) { Log.Info(m); });
                long temp = Cleanup.CleanTemp(1);
                Log.Info(string.Format("Уборка: освобождено {0} (копии) + {1} (временные файлы)", Cleanup.FormatSize(freed), Cleanup.FormatSize(temp)));
                return;
            }
            if (restoreUacMode)
            {
                bool done = UacTools.RestoreDefaults();
                Log.Info(done ? "UAC: восстановлены стандартные значения (5/1)." : "UAC: не удалось восстановить (нужны права администратора).");
                return;
            }
            if (checkSelfMode)
            {
                SelfUpdateInfo check = SelfUpdate.Check(true);
                Log.Info(string.Format("Самообновление: локальная {0}, релиз {1}, доступна {2}, ok={3}, error={4}",
                    check.Local, check.Remote, check.Available, check.Ok, check.Error));
                return;
            }
            if (!string.IsNullOrEmpty(msgBoxShot))
            {
                Theme.Apply("dark");
                DarkModeTest.Run(msgBoxShot);
                return;
            }
            if (!string.IsNullOrEmpty(appWatch))
            {
                int watchPid;
                if (!int.TryParse(appWatch, out watchPid))
                {
                    Log.Error("--app-watch: нужен PID процесса.");
                    return;
                }
                AppConfig watchConfig = AppConfig.Load(AppConfig.DefaultPath());
                ZapretInstall watchInstall = ZapretInstall.Detect(string.IsNullOrEmpty(rootArg) ? watchConfig.Get("Root", "") : rootArg);
                Log.Info("Проверка приложения: наблюдаем за PID " + watchPid + " 20 с.");
                System.Collections.Generic.List<string> watchIps;
                System.Collections.Generic.List<string> watchHosts = AppProbe.Watch(watchPid, 20, null, out watchIps);
                Log.Info("Проверка приложения: доменов " + watchHosts.Count + ", адресов " + watchIps.Count + " — " + string.Join(", ", watchHosts.ToArray()));
                System.Collections.Generic.List<string> okHosts;
                System.Collections.Generic.List<string> watchBlocked = AppProbe.BlockedHosts(watchHosts, null, out okHosts);
                Log.Info("Проверка приложения: заблокировано " + watchBlocked.Count + " — " + string.Join(", ", watchBlocked.ToArray()));
                if (watchBlocked.Count > 0)
                {
                    int watchAdded;
                    string watchPath;
                    string watchMessage = SiteDoctor.AddToUserList(watchInstall, watchBlocked, out watchAdded, out watchPath);
                    Log.Info("Проверка приложения: " + watchMessage);
                }
                return;
            }
            if (tgPrepareMode)
            {
                Log.Info("Режим подготовки тихого TG WS Proxy.");
                bool prepared;
                string message = TgSource.Prepare(TgSource.LatestRef(), null, out prepared);
                Log.Write(prepared ? LogLevel.Ok : LogLevel.Error, "Тихий режим TG WS Proxy: " + message);
                return;
            }
            StringBuilder argFlags = new StringBuilder();
            foreach (string arg in args)
            {
                if (!arg.StartsWith("--", StringComparison.Ordinal)) continue;
                int eq = arg.IndexOf('=');
                argFlags.Append(eq > 0 ? arg.Substring(0, eq) : arg).Append(' ');
            }
            Log.Info("Аргументы: " + args.Length + " шт. [" + argFlags.ToString().Trim() + "]");
            Log.Info("Права администратора: " + (Native.IsAdmin() ? "да" : "нет") + " / " + (limited ? "ограниченный режим" : "обычный режим"));

            AppConfig config = AppConfig.Load(AppConfig.DefaultPath());
            float uiScale = 1f;
            float.TryParse(config.Get("UiScale", "100"), out uiScale);
            Theme.InitFonts(uiScale / 100f);
            Theme.Apply(config.Get("Theme", "auto"));
            string preferred = rootArg;
            if (string.IsNullOrEmpty(preferred)) preferred = config.Get("Root", "");
            ZapretInstall install = ZapretInstall.Detect(preferred);
            Log.Info("Папка zapret: " + install.Root + (install.IsValid ? " (версия " + install.Version + ")" : " (не найдена)"));

            Shell shell = new Shell();
            shell.Config = config;
            shell.Install = install;
            shell.Ops = new ZapretOps(install);
            if (install.IsValid && config.Get("Root", "") != install.Root)
            {
                config.Set("Root", install.Root);
                config.Save();
            }

            MainForm form = new MainForm(shell);
            int w = config.GetInt("WindowW", 1280);
            int h = config.GetInt("WindowH", 880);
            int x = config.GetInt("WindowX", int.MinValue);
            int y = config.GetInt("WindowY", int.MinValue);
            form.Width = Math.Max(form.MinimumSize.Width, w);
            form.Height = Math.Max(form.MinimumSize.Height, h);
            if (x != int.MinValue && y != int.MinValue)
            {
                Rectangle wa = SystemInformation.VirtualScreen;
                if (x > wa.Left - 200 && y > wa.Top - 200 && x < wa.Right - 100 && y < wa.Bottom - 100)
                {
                    form.StartPosition = FormStartPosition.Manual;
                    form.Location = new Point(x, y);
                }
            }
            if (config.GetBool("Maximized", false)) form.WindowState = FormWindowState.Maximized;

            try
            {
                Application.Run(form);
            }
            finally
            {
                Log.Info("=== Завершение работы ===");
                if (_mutex != null) { try { _mutex.ReleaseMutex(); } catch { } }
            }
        }

        private static bool _mutexCreated;

        private static void ActivateExistingWindow()
        {
            // просим уже запущенный экземпляр показать окно
            try
            {
                using (EventWaitHandle ev = EventWaitHandle.OpenExisting("ZapretManager_Activate"))
                {
                    ev.Set();
                    Log.Info("Передал запрос на показ окна уже запущенному экземпляру.");
                }
            }
            catch { }
            for (int i = 0; i < 6; i++)
            {
                IntPtr h = Native.FindWindow(null, AppInfo.Title);
                if (h != IntPtr.Zero)
                {
                    Native.ShowWindow(h, 9);
                    Native.SetForegroundWindow(h);
                    return;
                }
                Thread.Sleep(300);
            }
        }

        private static void ReportCrash(Exception ex)
        {
            try
            {
                Log.Write(LogLevel.Error, "Необработанная ошибка: " + (ex == null ? "unknown" : ex.ToString()));
                MessageBox.Show("Произошла ошибка:\n\n" + (ex == null ? "неизвестная" : ex.Message) +
                    "\n\nПодробности записаны в журнал:\n" + Path.Combine(AppPaths.DataDir, "manager.log"),
                    AppInfo.Title, MessageBoxButtons.OK, MessageBoxIcon.Error);
            }
            catch { }
        }

        /// <summary>Служебный режим самообновления без интерфейса (--self-update).</summary>
        private static void RunSelfUpdateMode()
        {
            try
            {
                SelfUpdateInfo info = SelfUpdate.Check(true);
                Log.Info(string.Format("Самообновление: локальная {0}, в репозитории {1}, доступно={2}{3}",
                    info.Local, info.Remote, info.Available, info.Ok ? "" : " (" + info.Error + ")"));
                if (!info.Ok || !info.Available)
                {
                    Log.Info("Обновление не требуется.");
                    return;
                }
                bool ok;
                string pending;
                string msg = SelfUpdate.Apply(info, null, out ok, out pending);
                Log.Write(ok ? LogLevel.Ok : LogLevel.Error, "Самообновление: " + msg);
            }
            catch (Exception ex)
            {
                Log.Error("Самообновление: ошибка", ex);
            }
        }

        private static bool HasFlag(string[] args, string flag)
        {
            foreach (string a in args)
            {
                if (string.Equals(a, flag, StringComparison.OrdinalIgnoreCase)) return true;
            }
            return false;
        }

        private static string FindArgValue(string[] args, string name)
        {
            for (int i = 0; i < args.Length; i++)
            {
                if (string.Equals(args[i], name, StringComparison.OrdinalIgnoreCase))
                {
                    return i + 1 < args.Length ? args[i + 1] : null;
                }
                if (args[i].StartsWith(name + "=", StringComparison.OrdinalIgnoreCase))
                {
                    return args[i].Substring(name.Length + 1).Trim('"');
                }
            }
            return null;
        }

        /// <summary>Собирает аргументы для перезапуска с правами администратора.</summary>
        private static string BuildElevatedArgs(string[] args)
        {
            StringBuilder sb = new StringBuilder();
            sb.Append("--elevated");
            for (int i = 0; i < args.Length; i++)
            {
                string a = args[i];
                if (string.Equals(a, "--elevated", StringComparison.OrdinalIgnoreCase)) continue;
                if (string.Equals(a, "--limited", StringComparison.OrdinalIgnoreCase)) continue;
                sb.Append(' ');
                if (a.IndexOf(' ') >= 0) sb.Append('"').Append(a).Append('"');
                else sb.Append(a);
            }
            return sb.ToString();
        }
    }

    public static class AppIcon
    {
        public static Icon Get()
        {
            try { return Icon.ExtractAssociatedIcon(Application.ExecutablePath); }
            catch { return SystemIcons.Application; }
        }
    }
}
